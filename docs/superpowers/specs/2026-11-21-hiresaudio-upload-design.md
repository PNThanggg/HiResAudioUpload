# HiResAudioUpload — Design

## Context

`D:\Code\Tools\ProjectHiResAudio\` has two siblings:
- `HiResAudio/` — Android Jetpack Compose app (package `audios.hi_res`), currently a fresh template. It will play music streamed from Cloudflare R2.
- `HiResAudioUpload/` — empty. This is where the new tool goes.

**Intent:** the user needs an admin tool to manage the music library that the HiResAudio Android app plays. They upload MP3s to R2, then view, edit, and delete tracks. Before a file is stored, it is analysed (loudness, true peak, gapless, real bandwidth, waveform) so the app can apply track gain and gapless playback and show an honest quality label. The Android app must discover tracks **without holding R2 credentials**, so the tool also publishes a public **catalog** that the app reads.

Requested by the user: Rust, UI built with the `ui-ux-pro-max:design` skill, upload MP3 to R2, basic CRUD UI. The user supplied a "Logic core" document (MP3 processing rules, referred to below as **LC §n**). It is the normative spec for processing. This design adapts it from a server pipeline to a desktop app.

## Goals
- Upload one or many MP3s (drag-drop or file picker). Each file goes through the LC pipeline: validate → dedupe → gapless → single-decode analysis → served file → cover → upload → catalog.
- Library list: cover thumbnail, title, artist, album, duration, quality label (`qualityHint`, e.g. "MP3 320 · Full band" / "MP3 320 · nguồn ~128k"), LUFS, size.
- Track detail view: all metadata, including the waveform.
- Edit: title, artist, album, album_artist, date, genre, track, disc. Replace the cover.
- Delete one or more tracks, with confirmation.
- Settings screen for R2 credentials, with a "Test connection" button.

## Non-goals (v1)
- Changes to the Android app. LC §9 (track gain, gapless trimming, download/cache) is the app's job. This spec only defines the data it consumes.
- Lossless inputs and Opus variants (LC §11 "sau này"). Re-encoding of any kind (LC §0).
- Re-analysis when `analysisVersion` bumps. The field is stored, but there is no "re-analyse library" action in v1.
- Playlists, search, in-tool playback, multi-user, auth, hosting.
- Orphan-object reconciliation.

## Anti-goals
- Modifying audio samples (LC §0). A served file must decode to bit-identical PCM.
- R2 keys reachable by the app or the public.
- A catalog entry pointing at a missing object. A UI success means the objects and the catalog were both written.
- A generic-looking admin UI.

## Constraints / environment
- ffmpeg (with libmp3lame) is **not installed**. It is needed only at dev time to generate fixtures and oracle values (e.g. `winget install Gyan.FFmpeg`, with user OK). The committed fixtures mean tests then run without it.
- Windows 11, rustc 1.99.1, Node 24 + npm 11 (no pnpm). Tauri CLI not installed: use the `@tauri-apps/cli` npm devDependency. WebView2 ships with Win11.
- The R2 bucket needs public access (r2.dev or custom domain). Range requests work by default. Do not enable compression for audio/images (LC §7).
- `HiResAudioUpload/` is not a git repo yet: `git init`.

## Approaches considered
| | Approach | Trade-off |
|---|---|---|
| A | Local web app (Axum + browser) | Simplest, but no native window |
| **B (user's pick)** | **Tauri 2 desktop**: Rust core + web UI in a native window | Real app with an installer, at the cost of extra Tauri setup |
| C | Pure-Rust GUI (egui/Slint) | Rejected: can't reach the design quality of `ui-ux-pro-max` |

**Processing engine** (user picked pure Rust):
- **Pure Rust (chosen):** `symphonia` (probe + decode), `ebur128` crate (integrated LUFS, LRA, true peak with ×4 oversampling: the same algorithm as the ffmpeg `ebur128` filter), `realfft` (bandwidth), a hand-written LAME/Xing parser, and `lofty` (tags + cover bytes). The LC §6 "remux" becomes **stripping the ID3v2 header + trailing ID3v1/APEv2 bytes**: the MPEG frames, including the Xing/LAME frame, are copied byte-for-byte, so PCM and LAME delay/padding are unchanged by construction (still verified). This gives a single exe with no sidecar.
- ffmpeg sidecar: matches the LC doc's tooling literally, but bundles roughly 80 MB+ and needs process management and timeout kill.

### How LC maps to a desktop app
| LC | Here |
|---|---|
| §1 staging bucket, presigned URL, API, queue | Not needed. The file is already local, and the Rust core processes it directly |
| §1 workers, §11 worker count = cores | Analysis runs in parallel up to the number of cores (`spawn_blocking`). Uploads and catalog writes are serialized |
| §11 container sandbox | N/A: these are the user's own files on their own machine. Keep panic isolation: one file's failure doesn't kill the batch |
| §2 hard timeout 5 min | Per-file timeout. On expiry the file is marked `TIMEOUT` and its result discarded |
| §1/§11 "DB" | `catalog.json` + `meta/<id>.json` on R2 |
| §1/§11 "storage first, DB last" | Same: objects first, then the catalog |
| §8 waveform only in detail endpoint | `catalog.json` omits the waveform. `meta/<id>.json` holds the full record incl. the waveform |
| §11 error classes | `INVALID_FORMAT`, `TOO_LARGE`, `TOO_SHORT`, `SILENT`, `TIMEOUT`, `INTERNAL`, plus `DUPLICATE` (not an error: shows the existing track) |

## Design

### Architecture
```
HiResAudioUpload/
  src-tauri/src/
    pipeline/
      probe.rs       LC §2 validation (symphonia probe; never trusts extension)
      lame.rs        LC §4 ID3 skip (syncsafe), first frame, Xing flags, LAME delay/padding → trimStart/trimEnd/totalSamples
      analyze.rs     LC §5 single decode pass feeding 3 sinks: ebur128, spectrum accumulator, waveform
      bandwidth.rs   LC §5.2 (8192 Hann frames, 1 per 32768 samples, 100 Hz bands, ref = 2–8 kHz, −75 dB from Nyquist, hint table)
      waveform.rs    LC §5.3 (1024-sample block peaks → 800 points, clamp, round 3)
      serve.rs       LC §6 byte-original vs tag-strip (cover > 200 KB) + PCM-hash & LAME equality checks
      cover.rs       LC §7 extract attached picture as-is (jpg/png only)
      mod.rs         orchestration + timeout + error classes; returns TrackRecord
    r2.rs            aws-sdk-s3, endpoint https://<account>.r2.cloudflarestorage.com, region "auto"
    catalog.rs       catalog model + pure ops over a Storage trait
    settings.rs      settings JSON in the Tauri app config dir
    commands.rs      #[tauri::command] surface + progress events
  src/               Vite + TypeScript, plain CSS design tokens from the design pass
```

### R2 layout (contract for the Android app)
| Key | Content-Type | Cache-Control |
|---|---|---|
| `audio/<id>.mp3` (`id` = SHA-256 of the served file, LC §3) | `audio/mpeg` | `public, max-age=31536000, immutable` |
| `cover/<coverSha256>.<jpg\|png>` | `image/jpeg` / `image/png` | `public, max-age=31536000, immutable` |
| `meta/<id>.json` (full record incl. waveform) | `application/json` | `no-cache` |
| `catalog.json` | `application/json` | `no-cache` |

Deviation from LC §7: covers are keyed by **the cover's own hash**, not the track id. Because covers are cached as immutable and the user can replace a cover in Edit, a new cover needs a new key.

`catalog.json` = `{ version: 1, updatedAt, tracks: [TrackSummary] }`, where TrackSummary = all LC §8 fields **except `waveform`**: `id, sourceSha256, audioKey, coverKey, audioUrl, coverUrl, sizeBytes, codec, bitrate, vbr, sampleRate, channels, durationMs, title, artist, album, albumArtist, date, genre, track, disc, integratedLufs, truePeakDbtp, lra, effectiveBandwidthHz, qualityHint, trimStart, trimEnd, totalSamples (null if no LAME), analysisVersion (=1), createdAt, updatedAt`.

### Operations
- **Create** (per file): LC pipeline → dedupe on `sourceSha256` against the catalog (hit = `DUPLICATE`, show the existing track) → put audio, cover, meta → save catalog. If the catalog save fails, delete the objects just written.
- **Read:** catalog via S3 GetObject (avoids CDN staleness). Detail view loads `meta/<id>.json`.
- **Update:** tag fields + optional new cover (upload under the new hash key) → rewrite `meta/<id>.json` → save catalog → delete the old cover object if no other track references it. Audio is never touched.
- **Delete (multi):** remove from catalog and save first, then delete audio/meta/unreferenced covers. An orphaned object is harmless, but a dangling catalog entry breaks the app.
- The catalog is read-modify-written under an in-process mutex (single admin).

### UI (designed with `ui-ux-pro-max:design` before frontend work)
**Library** (table: cover, title, artist, album, duration, quality badge, LUFS, size; multi-select + bulk delete; empty and loading states). **Upload panel** (drop zone + queue with per-file stage: analysing → uploading → done / duplicate / error code). **Track detail** (waveform, loudness/peak/LRA, bandwidth + hint, gapless numbers, technical info). **Edit dialog**. **Delete confirm**. **Settings** (account id, access key id, secret, bucket, public base URL, Test connection). First run without settings → Settings.

### Settings / secrets
JSON in the Tauri app config dir. No OS keychain in v1 (builder call: personal machine).

### Testing
- Fixtures (generated once with ffmpeg/lame **at dev time only**, committed): the LC §10 set (30 s stereo pink noise, 44.1 kHz: 320 CBR, 128→320 re-encode, loud + limiter, V2, 128 CBR), plus a file with a > 200 KB cover, a silent file, a 0.5 s file, and a non-MP3 renamed `.mp3`.
- No original §10 fixtures exist (user confirmed), so **ffmpeg is the oracle**: a dev-time script generates the fixtures and records ffmpeg `ebur128=peak=true` results next to them. Unit tests assert Rust vs those recorded values (LUFS/TP/LRA ±0.3) and an exact `qualityHint` per fixture (320 CBR → full, 128→320 → medium, V2 → high, 128 CBR → medium), plus gapless 1105 / 695 / 1 323 000 exactly. The §10 numbers themselves are reference only.
- Also assert tag-strip PCM-hash equality + unchanged LAME delay/padding, error classes for bad inputs, and catalog ops against an in-memory Storage (add / dup / update with cover swap / delete / rollback).
- Perf check: a 4-min track is analysed in ≲ 3–4 s in a release build.
- Manual E2E against a real bucket: upload → `curl` catalog.json/meta → edit + cover swap → delete; audio URL supports `Range`.

## Calls made after the builder review (designer's calls, user may override)
- **Analysis runs on gapless-trimmed PCM** (symphonia `enable_gapless`, LAME delay/padding applied), matching ffmpeg's default and what the app plays. Loudness, peak, waveform and bandwidth all use the trimmed signal. `durationMs = totalSamples / sampleRate` when gapless data exists, otherwise the decoded length. The served-vs-source PCM-hash check uses the same mode on both sides.
- **The quality label is UI-only, not stored.** The catalog keeps `qualityHint` (LC §8), and the label is derived from `bitrate` + hint: full → "Full band", high → "High (~V0–V2)", medium → "nguồn ~128k" when bitrate ≥ 192 else "Medium", low → "Nguồn kém".
- **Two sources that produce the same served file (same `id`)** → `DUPLICATE`, showing the existing track. One audio object maps to exactly one catalog entry, so Delete never needs to count references to audio.
- **Dedupe order:** check `sourceSha256` first (cheap, before decode). The `id` collision check comes after the served file is built.

## Left to the builder
Crate versions; byte vs stage-level upload progress; frontend file layout; error message wording; icon; window size; exact tolerances if fixtures show the Rust measurements differ slightly from ffmpeg.

## Next step after approval
`git init` `HiResAudioUpload/`, save this doc to `docs/superpowers/specs/2026-11-21-hiresaudio-upload-design.md` and the user's Logic core doc verbatim to `docs/logic-core.md` (the LC § references point there), commit, then invoke `superpowers:writing-plans`.
