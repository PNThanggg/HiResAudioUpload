# HiResAudioUpload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri 2 desktop app that analyses MP3s per the Logic core (no re-encode), uploads them to Cloudflare R2, and manages the library (list / detail / edit / delete) through a public `catalog.json` that the HiResAudio Android app reads.

**Architecture:** A Rust lib crate inside `src-tauri/` holds three layers: `pipeline/` (pure, file-in → `ProcessedTrack`-out, no I/O beyond the input bytes), `catalog.rs` (library CRUD over an async `Storage` trait, with an in-memory impl for tests), and `r2.rs` (S3 impl). `commands.rs` is the thin Tauri surface. The frontend is Vite + vanilla TypeScript. It talks to Rust only through `src/api.ts`, which falls back to a mock when it runs outside Tauri, so the UI can be checked in a plain browser.

**Tech Stack:** Tauri 2, `tauri-plugin-dialog`, tokio, anyhow, symphonia (mp3), lofty, ebur128, realfft, sha2, aws-sdk-s3 + aws-config, async-trait, serde/serde_json, thiserror, time; Vite + TypeScript + vitest. Builder picks the current stable versions.

**Spec:** `docs/superpowers/specs/2026-11-21-hiresaudio-upload-design.md`. Processing rules are in `docs/logic-core.md` (cited as LC §n). Read both before starting any task.

## Global Constraints
- Never re-encode or modify audio samples. A served file is the source bytes, or the source with tag bytes stripped (LC §0, §6).
- Format is always detected from content, never from the extension (LC §2).
- Error codes (exact strings): `INVALID_FORMAT`, `TOO_LARGE`, `TOO_SHORT`, `SILENT`, `TIMEOUT`, `INTERNAL`. `DUPLICATE` is an outcome, not an error. Duration > 2 h maps to `TOO_LARGE` (planner's call: there is no separate code).
- Limits: size ≤ 200 MB (200 × 1024 × 1024 bytes); duration ≥ 1 s and ≤ 2 h; mono/stereo; sample rate ∈ {32000, 44100, 48000} for MPEG-1, {8000, 11025, 12000, 16000, 22050, 24000} for MPEG-2/2.5; per-file timeout 300 s; remux when the embedded cover is > 200 × 1024 bytes.
- R2 keys and headers: `audio/<id>.mp3` (`audio/mpeg`) and `cover/<coverSha256>.<jpg|png>` (`image/jpeg`/`image/png`), both `Cache-Control: public, max-age=31536000, immutable`. `meta/<id>.json` and `catalog.json` are `application/json` with `Cache-Control: no-cache`.
- `id` = lowercase hex SHA-256 of the served file. `sourceSha256` = lowercase hex SHA-256 of the source file. `analysisVersion` = 1.
- JSON is camelCase. Timestamps are RFC 3339 UTC.
- Write order: objects first, catalog last. Delete order: catalog first, objects after.
- R2 secrets never leave the Rust side. Commands return settings with the secret masked.
- Tauri identifier `audios.hi_res.upload`, product name `HiResAudio Upload`.
- Frontend Composables rule from the user's CLAUDE.md does not apply (no Compose here).

## Review Focus
1. **Unusual ID3v2 headers:** files with an ID3v2 tag that has padding or junk bytes before the first frame, or an `ID3` footer flag. The first frame must still be found, so gapless data and the strip stay correct. → Task 3 test `finds_frame_after_padding_and_footer`.
2. **Bad or missing catalog:** a fresh bucket with no `catalog.json` must give an empty library. A `catalog.json` that isn't valid JSON must surface an error and **must never be overwritten** by a later add or delete. → Task 8 tests `missing_catalog_is_empty`, `corrupt_catalog_blocks_writes`.
3. **One failure in a batch:** when one file fails mid-batch (network error on its upload), the other files still finish, the catalog lists only the successes, and nothing dangles. → Task 8 test `failed_put_leaves_catalog_unchanged` + Task 10 batch loop continues on error.
4. **Vietnamese text and missing titles:** diacritics in tags and file names round-trip unchanged. A file with no title tag gets the file stem as its title. → Task 5 test `title_falls_back_to_file_stem_unicode`.
5. **Sloppy settings input:** a public base URL with a trailing slash, or credentials with surrounding whitespace, must not produce `//` in URLs or auth failures. → Task 9 test `settings_normalize_trims_and_strips_slash`.

---

### Task 1: Scaffold the Tauri app

**Files:**
- Create: everything from `create-tauri-app` (vanilla-ts) at the repo root, keeping `docs/`
- Create: `.gitignore` (node_modules, dist, src-tauri/target, `.env`)
- Modify: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`
- Create: empty modules `src-tauri/src/{error.rs,catalog.rs,r2.rs,settings.rs,commands.rs}`, `src-tauri/src/pipeline/{mod.rs,lame.rs,serve.rs,probe.rs,analyze.rs,bandwidth.rs,waveform.rs}`

**Interfaces:**
- Produces: the lib crate `hires_upload_lib`, with all modules above `pub` so integration tests in `src-tauri/tests/` can reach them.

- [ ] **Step 1:** Scaffold into a temp dir with `npm create tauri-app@latest hiresaudio-upload -- --template vanilla-ts --manager npm --yes`, then move the contents into the repo root (keep `.git` and `docs/`). Set identifier and product name (Global Constraints). Add the `@tauri-apps/cli` devDependency and the `vitest` devDependency with an `"test": "vitest run"` script.
- [ ] **Step 2:** Add the Rust deps from Tech Stack to `src-tauri/Cargo.toml` (symphonia with features `mp3`; tokio with `rt-multi-thread`, `macros`, `sync`, `time`; aws-sdk-s3 with `behavior-version-latest`). Register `tauri-plugin-dialog` in `lib.rs`. Declare the module tree.
- [ ] **Step 3: Verify:** `cd src-tauri && cargo check` succeeds. `npm run build` succeeds. `npm run tauri dev` opens a window (close it).
- [ ] **Step 4: Commit:** `🎉 Scaffold Tauri 2 app`

---

### Task 2: Test fixtures and ffmpeg oracle

**Files:**
- Create: `scripts/gen-fixtures.sh` (Git Bash)
- Create: `src-tauri/tests/fixtures/*.mp3`, `src-tauri/tests/fixtures/oracle.json` (committed)

**Interfaces:**
- Produces: the fixture files below, plus `oracle.json` = `{ "<file>": { "lufs": f64, "lra": f64, "truePeakDbtp": f64 } }` for every analysable fixture. ffmpeg decodes gapless by default, which is what the spec's calls require.

- [ ] **Step 1:** ffmpeg is not installed. **Ask the user** before running `winget install Gyan.FFmpeg` (it must include libmp3lame).
- [ ] **Step 2:** Write `gen-fixtures.sh`. Source: `-f lavfi -i "anoisesrc=color=pink:sample_rate=44100:amplitude=0.1:duration=30:seed=42" -ac 2`. Produce:
  | File | Recipe |
  |---|---|
  | `pink_320.mp3` | `-c:a libmp3lame -b:a 320k` |
  | `pink_128to320.mp3` | encode 128k, then re-encode that file at 320k |
  | `pink_loud.mp3` | `-af "volume=14dB,alimiter=limit=0.98:level=false"`, 320k |
  | `pink_v2.mp3` | `-q:a 2` |
  | `pink_128.mp3` | `-b:a 128k` |
  | `cover_big.mp3` | `pink_320` + attached JPEG > 200 KB (1600×1600 `geq=random(1)*255` noise, `-q:v 1`), `-disposition:v attached_pic -id3v2_version 3`, tags `title="Bài hát thử" artist="Nghệ sĩ" album="Album thử"` |
  | `cover_small.mp3` | `pink_320` + attached 64×64 PNG (< 200 KB) |
  | `noxing_notag.mp3` | 5 s, `-write_xing 0 -map_metadata -1`, 192k |
  | `silent.mp3` | `anullsrc=r=44100:cl=stereo` 5 s, 128k |
  | `short.mp3` | pink 0.5 s, 128k |
  | `fake.mp3` | a WAV file renamed |

  For each analysable file, run `ffmpeg -nostats -i F -af ebur128=peak=true -f null -` and parse the Summary `I:`, `LRA:` and true-peak `Peak:` lines into `oracle.json`.
- [ ] **Step 3: Verify:** run the script. Check that `cover_big.mp3` is > 1.4 MB and that `oracle.json` has the 7 pink/cover entries with `pink_loud.truePeakDbtp > 0`. If it isn't > 0, raise the volume in the recipe and regenerate.
- [ ] **Step 4: Commit:** `✅ Add MP3 test fixtures and ffmpeg loudness oracle`

---

### Task 3: MPEG frame + LAME gapless parser (`pipeline/lame.rs`)

**Files:**
- Modify: `src-tauri/src/pipeline/lame.rs`
- Test: `src-tauri/tests/lame.rs`

**Interfaces:**
- Produces:
  - `pub fn id3v2_len(b: &[u8]) -> usize`: 0 if no `ID3` header, else 10 + syncsafe(b[6..10]) + (10 if b[5] & 0x10).
  - `pub struct FrameHeader { pub offset: usize, pub mpeg1: bool, pub channels: u8, pub sample_rate: u32, pub bitrate_kbps: u32, pub samples_per_frame: u32, pub side_info_len: usize }`
  - `pub fn first_frame(b: &[u8]) -> Option<FrameHeader>`: scans from `id3v2_len(b)` for an 11-bit sync with layer III, bitrate index ∉ {0, 15}, sample-rate index ≠ 3. It accepts a candidate only if a second valid header follows at the computed frame length (or the file ends there), so random bytes don't match.
  - `pub struct Gapless { pub encoder: String, pub vbr: bool, pub frames: u32, pub delay: u32, pub padding: u32, pub trim_start: u32, pub trim_end: u32, pub total_samples: u64 }` (`vbr` = tag was `Xing`, not `Info`)
  - `pub fn read_gapless(b: &[u8]) -> Option<Gapless>`: LC §4 steps 4–6 and formulas. Returns `None` if there is no Xing/Info, no frame count, or no LAME tag.

- [ ] **Step 1: Write failing tests**
```rust
#[test] fn syncsafe_and_footer() {
    let mut b = vec![b'I',b'D',b'3',4,0,0x10, 0,0,0x02,0x01]; // size = 2*128+1 = 257, footer flag
    b.resize(10 + 257 + 10, 0);
    assert_eq!(id3v2_len(&b), 277);
}
#[test] fn gapless_matches_lc_example() {          // all five 30 s pink fixtures
    for f in ["pink_320","pink_128to320","pink_loud","pink_v2","pink_128"] {
        let g = read_gapless(&fixture(f)).unwrap();
        assert_eq!(g.trim_start, 1105, "{f}");
        assert_eq!(g.total_samples, 1_323_000, "{f}");
        assert_eq!(g.trim_end, g.padding.saturating_sub(529));
    }
    assert!(!read_gapless(&fixture("pink_320")).unwrap().vbr);
    assert!(read_gapless(&fixture("pink_v2")).unwrap().vbr);
}
#[test] fn no_xing_is_none() { assert!(read_gapless(&fixture("noxing_notag")).is_none()); }
#[test] fn finds_frame_after_padding_and_footer() {
    // Review Focus 1: ID3 tag (footer flag) + 3000 zero bytes, then the frames of pink_128
    let src = fixture("pink_128"); let audio = &src[id3v2_len(&src)..];
    let mut b = vec![b'I',b'D',b'3',4,0,0x10,0,0,0,10]; b.resize(30, 0); // 10 hdr + 10 body + 10 footer
    b.extend(std::iter::repeat(0u8).take(3000)); b.extend_from_slice(audio);
    assert_eq!(read_gapless(&b), read_gapless(&src));
    assert_eq!(first_frame(&b).unwrap().bitrate_kbps, 128);
}
#[test] fn garbage_is_none() { assert!(first_frame(&[0xFF; 64]).is_none()); assert!(read_gapless(b"hello").is_none()); }
```
(`fixture(name)` = read `tests/fixtures/{name}.mp3`. Put it in `tests/common/mod.rs` for reuse.)
- [ ] **Step 2: Run:** `cd src-tauri && cargo test --test lame`. Expected: FAIL (functions missing).
- [ ] **Step 3: Implement** the functions above. Use the MPEG bitrate and sample-rate tables for MPEG-1 / MPEG-2 / MPEG-2.5 layer III. Side info and samples-per-frame come from the LC §4 table. Frame length = `144000*kbps/sr + pad` for MPEG-1 and `72000*kbps/sr + pad` for MPEG-2/2.5. LAME delay/padding: `delay = (b21<<4)|(b22>>4)`, `padding = ((b22&0x0F)<<8)|b23`.
- [ ] **Step 4: Run:** `cargo test --test lame`. Expected: PASS. If `trim_start`/`total_samples` differ on the generated fixtures, stop and report the values. Don't loosen the test.
- [ ] **Step 5: Commit:** `✨ Parse MPEG frame header and LAME gapless info`

---

### Task 4: Tag stripping for the served file (`pipeline/serve.rs`, part 1)

**Files:**
- Modify: `src-tauri/src/pipeline/serve.rs`
- Test: `src-tauri/tests/serve.rs`

**Interfaces:**
- Consumes: `id3v2_len`, `first_frame` (Task 3)
- Produces:
  - `pub fn strip_tags(b: &[u8]) -> Vec<u8>`: returns `b[first_frame.offset .. end]`, where `end` excludes a trailing ID3v1 (`TAG`, last 128 bytes) and an APEv2 tag (`APETAGEX` footer, 32 bytes, plus its size field; checked before and after ID3v1).
  - `pub fn audio_payload_len(b: &[u8]) -> usize` = `strip_tags(b).len()` without allocating (used for the VBR average bitrate).

- [ ] **Step 1: Write failing tests:** `strip_removes_id3v2_and_cover` (`cover_big` → result starts with `0xFF` and is < 40 % of the source length; `read_gapless(stripped) == read_gapless(source)`), `strip_removes_id3v1_and_ape` (pink_128 frames + appended `APETAGEX` footer block + 128-byte `TAG…` → equals the frames only), `strip_is_identity_on_bare_frames` (stripping twice gives the same bytes).
- [ ] **Step 2: Run:** `cargo test --test serve`. Expected: FAIL.
- [ ] **Step 3: Implement** `strip_tags` and `audio_payload_len`.
- [ ] **Step 4: Run:** `cargo test --test serve`. Expected: PASS.
- [ ] **Step 5: Commit:** `✨ Strip ID3/APE tags without touching MPEG frames`

---

### Task 5: Probe, tags, cover, error codes (`error.rs`, `pipeline/probe.rs`)

**Files:**
- Modify: `src-tauri/src/error.rs`, `src-tauri/src/pipeline/probe.rs`
- Test: `src-tauri/tests/probe.rs`

**Interfaces:**
- Produces:
  - `error.rs`: `#[derive(Serialize)] #[serde(rename_all="SCREAMING_SNAKE_CASE")] pub enum ErrorCode { InvalidFormat, TooLarge, TooShort, Silent, Timeout, Internal }` and `pub struct PipelineError { pub code: ErrorCode, pub message: String }` (thiserror, Serialize).
  - `pub const MAX_BYTES: u64 = 200*1024*1024;`
  - `pub struct Tags { pub title: String, pub artist: Option<String>, pub album: Option<String>, pub album_artist: Option<String>, pub date: Option<String>, pub genre: Option<String>, pub track: Option<String>, pub disc: Option<String> }`
  - `pub struct Cover { pub bytes: Vec<u8>, pub ext: &'static str /* "jpg"|"png" */, pub mime: &'static str }`
  - `pub struct Probed { pub header: FrameHeader, pub gapless: Option<Gapless>, pub tags: Tags, pub cover: Option<Cover> }`
  - `pub fn probe(bytes: &[u8], file_stem: &str) -> Result<Probed, PipelineError>`: no `first_frame` → `INVALID_FORMAT`. Channels must be 1|2 and the sample rate in the Global Constraints set, else `INVALID_FORMAT`. If gapless is present and `total_samples / sr > 7200` → `TOO_LARGE`. Tags via lofty (read from an in-memory cursor; `ItemKey`s for the 8 fields, values trimmed, empty → None). Title falls back to `file_stem`. Cover = the front-cover picture, else the first picture, kept only if the magic bytes are JPEG (`FF D8 FF`) or PNG (`89 50 4E 47`).
  - `pub fn check_size(len: u64) -> Result<(), PipelineError>` (`TOO_LARGE` if > MAX_BYTES). The caller runs it on the file metadata before reading the file.
  - `pub fn sniff_image(b: &[u8]) -> Option<(&'static str, &'static str)>` (ext, mime). Also used by Task 8 for replacement covers.

- [ ] **Step 1: Write failing tests**
```rust
#[test] fn fake_mp3_is_invalid() { assert_eq!(probe(&fixture("fake"), "fake").unwrap_err().code, ErrorCode::InvalidFormat); }
#[test] fn reads_unicode_tags_and_big_cover() {
    let p = probe(&fixture("cover_big"), "cover_big").unwrap();
    assert_eq!(p.tags.title, "Bài hát thử"); assert_eq!(p.tags.artist.as_deref(), Some("Nghệ sĩ"));
    let c = p.cover.unwrap(); assert_eq!(c.ext, "jpg"); assert!(c.bytes.len() > 200*1024);
}
#[test] fn small_png_cover() { assert_eq!(probe(&fixture("cover_small"), "x").unwrap().cover.unwrap().ext, "png"); }
#[test] fn title_falls_back_to_file_stem_unicode() {      // Review Focus 4
    let p = probe(&fixture("noxing_notag"), "Nhạc Trịnh – Diễm xưa").unwrap();
    assert_eq!(p.tags.title, "Nhạc Trịnh – Diễm xưa"); assert!(p.gapless.is_none()); assert!(p.cover.is_none());
}
#[test] fn size_limit() { assert!(check_size(MAX_BYTES).is_ok()); assert_eq!(check_size(MAX_BYTES+1).unwrap_err().code, ErrorCode::TooLarge); }
```
- [ ] **Step 2: Run:** `cargo test --test probe`. Expected: FAIL.
- [ ] **Step 3: Implement** the functions above.
- [ ] **Step 4: Run:** `cargo test --test probe`. Expected: PASS.
- [ ] **Step 5: Commit:** `✨ Probe MP3, read tags and cover, define error codes`

---

### Task 6: Single-pass analysis (`analyze.rs`, `bandwidth.rs`, `waveform.rs`)

**Files:**
- Modify: `src-tauri/src/pipeline/{analyze.rs,bandwidth.rs,waveform.rs}`
- Test: `src-tauri/tests/analyze.rs` + unit tests inside `waveform.rs` and `bandwidth.rs`

**Interfaces:**
- Produces:
  - `waveform.rs`: `pub struct Waveform` with `new(channels: usize) -> Self`, `push(&mut self, interleaved: &[f32])`, `finish(self) -> Vec<f32>`. Blocks are 1024 frames, and each block value is the max |s| over both channels. Exactly 800 output points: point `i` = max of blocks `[i*N/800, max((i+1)*N/800, i*N/800+1))` (integer division, index clamped to `N-1`), clamped to [0,1] and rounded to 3 decimals. If N = 0, return 800 zeros.
  - `bandwidth.rs`: `pub struct Spectrum` with `new(sample_rate: u32) -> Self`, `push(&mut self, interleaved: &[f32], channels: usize)`, `finish(self) -> Option<u32>` (`effectiveBandwidthHz`, `None` if the reference power < 1e-20); `pub fn quality_hint(bw: u32, sample_rate: u32) -> &'static str`.
  - `analyze.rs`: `pub struct Analysis { pub decoded_frames: u64, pub integrated_lufs: f64, pub true_peak_dbtp: f64, pub lra: f64, pub effective_bandwidth_hz: Option<u32>, pub quality_hint: Option<&'static str>, pub waveform: Vec<f32>, pub pcm_sha256: [u8; 32] }`; `pub fn analyze(bytes: &[u8], cancel: &AtomicBool) -> Result<Analysis, PipelineError>`. It decodes once with symphonia (`FormatOptions { enable_gapless: true, .. }`, converting to interleaved f32) and feeds every packet to ebur128 (`Mode::I | Mode::LRA | Mode::TRUE_PEAK`), `Spectrum`, `Waveform` and a SHA-256 over the f32 little-endian bytes. `cancel` is checked once per packet (→ `TIMEOUT`). Decode errors → `INVALID_FORMAT`. Fewer than 1 s of decoded frames → `TOO_SHORT`. Integrated loudness −inf or < −70 → `SILENT`. True peak = 20·log10(max over channels of the ebur128 true peak).

- Spectrum algorithm (LC §5.2, values pinned):
```
mono = mean of channels. Ring through the mono stream in periods of 32768 samples; the first 8192 of each period form one frame.
frame *= Hann(8192); X = rfft(frame); acc[k] += |X[k]|^2  (k = 0..=4096); count += 1
bin_hz = sr / 8192
band b covers [100b, 100(b+1)) Hz; level_db(b) = 10*log10(mean(acc[k]/count for bins whose centre k*bin_hz is in band) + 1e-30)
ref_db  = 10*log10(mean of acc[k]/count for bins with 2000 <= k*bin_hz < 8000)
walk b from floor((sr/2 - 1)/100) down to 0; first b with level_db(b) > ref_db - 75 → return min(100(b+1), sr/2)
hint: bw >= 19500 || bw >= sr/2 - 1000 → "full"; >= 18000 "high"; >= 15500 "medium"; else "low"
```
  A file shorter than one period still contributes its first ≤ 8192 samples, zero-padded.

- [ ] **Step 1: Write failing tests**
  - `waveform.rs` unit: `always_800_points` (N = 3 blocks → 800 values, all equal to their block max), `peak_uses_both_channels` (L = 0.2, R = −0.9 → 0.9), `clamps_and_rounds` (1.7 → 1.0, 0.12345 → 0.123).
  - `bandwidth.rs` unit: `hint_thresholds` (19500 → full, `quality_hint(15_100, 32_000)` → full via the Nyquist − 1 kHz rule, 19499 → high, 18000 → high, 17999 → medium, 15500 → medium, 15499 → low).
  - `tests/analyze.rs`:
```rust
#[test] fn matches_ffmpeg_oracle_and_hints() {
    let o = oracle();                         // tests/fixtures/oracle.json
    for (f, hint) in [("pink_320","full"),("pink_128to320","medium"),("pink_loud","full"),("pink_v2","high"),("pink_128","medium")] {
        let a = analyze(&fixture(f), &AtomicBool::new(false)).unwrap();
        assert!((a.integrated_lufs - o[f].lufs).abs() <= 0.3, "{f} lufs");
        assert!((a.true_peak_dbtp - o[f].true_peak_dbtp).abs() <= 0.3, "{f} tp");
        assert!((a.lra - o[f].lra).abs() <= 0.3, "{f} lra");
        assert_eq!(a.quality_hint, Some(hint), "{f} bw={:?}", a.effective_bandwidth_hz);
        assert_eq!(a.decoded_frames, 1_323_000, "{f} gapless trim");
        assert_eq!(a.waveform.len(), 800);
    }
    assert!(analyze(&fixture("pink_loud"), &AtomicBool::new(false)).unwrap().true_peak_dbtp > 0.0);
}
#[test] fn silent_and_short_rejected() {
    assert_eq!(analyze(&fixture("silent"), &AtomicBool::new(false)).unwrap_err().code, ErrorCode::Silent);
    assert_eq!(analyze(&fixture("short"),  &AtomicBool::new(false)).unwrap_err().code, ErrorCode::TooShort);
}
#[test] fn cancel_gives_timeout() { assert_eq!(analyze(&fixture("pink_320"), &AtomicBool::new(true)).unwrap_err().code, ErrorCode::Timeout); }
#[test] fn pcm_hash_stable_across_strip() {
    let src = fixture("cover_big"); let c = AtomicBool::new(false);
    assert_eq!(analyze(&src,&c).unwrap().pcm_sha256, analyze(&strip_tags(&src),&c).unwrap().pcm_sha256);
}
```
- [ ] **Step 2: Run:** `cargo test --test analyze` and `cargo test --lib`. Expected: FAIL.
- [ ] **Step 3: Implement** the three modules per the interfaces and the algorithm above.
- [ ] **Step 4: Run:** the same commands. Expected: PASS. If `decoded_frames ≠ 1_323_000`, symphonia's gapless trim differs from LAME's. Report it and trim manually using `Gapless.trim_start`/`total_samples` from Task 3 rather than changing the assertion. If a hint is off, print the bandwidth and report it. The spec allows adjusting tolerances only for loudness numbers.
- [ ] **Step 5: Commit:** `✨ Single-pass loudness, bandwidth and waveform analysis`

---

### Task 7: Pipeline orchestration (`pipeline/mod.rs`, `serve.rs` part 2)

**Files:**
- Modify: `src-tauri/src/pipeline/mod.rs`, `src-tauri/src/pipeline/serve.rs`
- Test: `src-tauri/tests/pipeline.rs`

**Interfaces:**
- Consumes: Tasks 3–6
- Produces:
  - `pub const REMUX_COVER_BYTES: usize = 200*1024; pub const TIMEOUT: Duration = Duration::from_secs(300); pub const ANALYSIS_VERSION: u32 = 1;`
  - `pub fn sha256_hex(b: &[u8]) -> String`
  - `pub struct ProcessedTrack { pub id: String, pub source_sha256: String, pub served: Vec<u8>, pub remuxed: bool, pub cover: Option<Cover>, pub tags: Tags, pub codec: &'static str /* "mp3" */, pub bitrate_kbps: u32, pub vbr: bool, pub sample_rate: u32, pub channels: u8, pub duration_ms: u64, pub analysis: Analysis, pub gapless: Option<Gapless> }`
  - `pub fn process(source: Vec<u8>, file_stem: &str, cancel: &AtomicBool) -> Result<ProcessedTrack, PipelineError>`: probe → analyze(source) → served = source if cover ≤ REMUX_COVER_BYTES or absent, else `strip_tags(source)`. When remuxed, verify `analyze(served).pcm_sha256 == source pcm_sha256` and `read_gapless(served) == gapless`; a mismatch → `INTERNAL`. bitrate = `header.bitrate_kbps` when not vbr, else `audio_payload_len*8 / duration_s / 1000` (rounded). duration_ms = `total_samples*1000/sr` if gapless is present, else `decoded_frames*1000/sr`. Duration > 2 h → `TOO_LARGE`.
  - `pub async fn process_with_timeout(path: PathBuf) -> Result<ProcessedTrack, PipelineError>`: `check_size(metadata.len)` → read the file → `spawn_blocking(process)` under `tokio::time::timeout(TIMEOUT)`. On timeout, set `cancel` and return `TIMEOUT`. A panic (JoinError) → `INTERNAL`.

- [ ] **Step 1: Write failing tests:** `byte_original_when_small_cover` (`cover_small`: `!remuxed`, `served == source`, `id == source_sha256`, cover ext png); `remux_when_big_cover` (`cover_big`: `remuxed`, `id != source_sha256`, `served.len() < source.len()`, gapless unchanged); `cbr_and_vbr_bitrate` (pink_320 → 320 and `!vbr`; pink_v2 → `vbr` and 150..=230); `duration_from_gapless` (pink_320 → 30000); `missing_file_is_internal` (`process_with_timeout` on a non-existent path → `INTERNAL`, no panic).
- [ ] **Step 2: Run:** `cargo test --test pipeline`. Expected: FAIL.
- [ ] **Step 3: Implement** the functions above.
- [ ] **Step 4: Run:** `cargo test --test pipeline`. Expected: PASS.
- [ ] **Step 5: Commit:** `✨ Orchestrate MP3 pipeline with remux verification and timeout`

---

### Task 8: Catalog model and library CRUD (`catalog.rs`)

**Files:**
- Modify: `src-tauri/src/catalog.rs`
- Test: `src-tauri/tests/catalog.rs`

**Interfaces:**
- Consumes: `ProcessedTrack`, `sha256_hex`, `sniff_image`, `ANALYSIS_VERSION`
- Produces:
  - `#[async_trait] pub trait Storage: Send + Sync { async fn put(&self, key: &str, body: Vec<u8>, content_type: &str, cache_control: &str) -> anyhow::Result<()>; async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>>; async fn delete(&self, key: &str) -> anyhow::Result<()>; }`
  - `pub struct MemoryStorage` (pub so tests and the UI mock can use it), with `fail_puts_matching: Mutex<Option<String>>` to inject a put failure for keys containing the given substring, and `keys() -> Vec<String>`.
  - `TrackSummary` (serde camelCase) with exactly the spec's catalog fields (Option for nullable ones). `TrackRecord { #[serde(flatten)] summary: TrackSummary, waveform: Vec<f32> }`. `Catalog { version: u32 /*1*/, updated_at: String, tracks: Vec<TrackSummary> }`.
  - `pub struct TagEdit { title: String, artist, album, album_artist, date, genre, track, disc: Option<String> }` (Deserialize camelCase). Empty strings are normalised to None. A blank title → `LibraryError::Validation`.
  - `pub enum AddOutcome { Added(TrackSummary), Duplicate(TrackSummary) }` (Serialize, tagged `{"kind":"added"|"duplicate","track":…}`).
  - `pub enum LibraryError { CatalogCorrupt(String), NotFound(String), Validation(String), Storage(String) }` (Serialize as `{code, message}`).
  - `pub struct Library { storage: Arc<dyn Storage>, public_base: String, lock: tokio::sync::Mutex<()> }` with:
    - `new(storage, public_base) -> Self`
    - `list() -> Result<Vec<TrackSummary>>`: missing catalog → empty; invalid JSON → `CatalogCorrupt`
    - `detail(id) -> Result<TrackRecord>` (reads `meta/<id>.json`)
    - `add(p: ProcessedTrack) -> Result<AddOutcome>`: under the lock, reload the catalog. Duplicate if `sourceSha256` or `id` already exists. Otherwise put audio → cover (key `cover/<sha256(cover)>.<ext>`) → meta → catalog. If the meta or catalog put fails, delete the audio and meta objects and any cover not referenced by the current catalog.
    - `update(id, edit: TagEdit, new_cover: Option<Vec<u8>>) -> Result<TrackSummary>`: cover bytes go through `sniff_image` (None → Validation). Put the new cover → put meta → save catalog → delete the old cover if no track references it any more. `coverUrl` is `{base}/{coverKey}`.
    - `delete(ids: &[String]) -> Result<()>`: save the catalog without the ids, then delete `audio/`, `meta/` and unreferenced covers. Object-delete failures are ignored after the catalog save.
  - URLs: `{public_base}/{key}`. `public_base` is already normalised (Task 9).

- [ ] **Step 1: Write failing tests** (they build `ProcessedTrack` via `process(fixture(..))`)
  - `missing_catalog_is_empty`: `list()` on empty storage → `[]`.
  - `add_writes_objects_with_headers_then_catalog`: keys `audio/<id>.mp3`, `cover/<hex>.png`, `meta/<id>.json`, `catalog.json` all exist. The audio put used `audio/mpeg` + `public, max-age=31536000, immutable`, and the catalog used `no-cache`. `catalog.json` has no `waveform` key, while `meta` has 800 values.
  - `same_file_twice_is_duplicate`: the second add → `Duplicate` with the same id. The catalog has 1 track.
  - `failed_put_leaves_catalog_unchanged` (Review Focus 3): set the failure on `"catalog.json"` → `add` Errs, and `keys()` contains no `audio/`, `meta/` or `cover/` keys. Then clear the failure, add two files, set the failure on `"audio/"`, add a third → Err, and `list()` still has 2 tracks.
  - `corrupt_catalog_blocks_writes` (Review Focus 2): put `b"{not json"` as `catalog.json` → `list`, `add` and `delete` all → `CatalogCorrupt`, and `get("catalog.json")` is still `{not json`.
  - `update_changes_tags_and_swaps_cover`: the title is changed. The new cover key differs from the old one, the old cover is deleted, and the meta is updated. A blank title → Validation. Non-image bytes → Validation.
  - `shared_cover_survives_single_delete`: two tracks get the same cover bytes via update. Deleting one keeps the cover object, and deleting both removes it. Also `audio/` and `meta/` are gone for deleted ids.
- [ ] **Step 2: Run:** `cargo test --test catalog`. Expected: FAIL.
- [ ] **Step 3: Implement** `catalog.rs`.
- [ ] **Step 4: Run:** `cargo test --test catalog`. Expected: PASS.
- [ ] **Step 5: Commit:** `✨ Library CRUD over storage with catalog and rollback`

---

### Task 9: Settings and the R2 storage (`settings.rs`, `r2.rs`)

**Files:**
- Modify: `src-tauri/src/settings.rs`, `src-tauri/src/r2.rs`
- Test: `src-tauri/tests/settings.rs`, `src-tauri/tests/r2_live.rs` (`#[ignore]`)

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, Clone)] #[serde(rename_all="camelCase")] pub struct Settings { pub account_id, pub access_key_id, pub secret_access_key, pub bucket, pub public_base_url: String }`
  - `Settings::normalize(self) -> Self` (trims all fields, strips trailing `/` from `public_base_url`); `Settings::is_complete(&self) -> bool`; `Settings::masked(&self) -> Settings` (the secret becomes `"••••" + last 4`, or `""` if empty)
  - `pub fn load(path: &Path) -> Settings` (missing or invalid file → default); `pub fn save(path: &Path, s: &Settings) -> io::Result<()>`. Path = `app.path().app_config_dir()/settings.json`.
  - `pub struct R2Storage` implementing `Storage`, with `pub async fn new(s: &Settings) -> Self` (endpoint `https://{account_id}.r2.cloudflarestorage.com`, region `auto`, static credentials, path-style off) and `pub async fn test_connection(&self) -> anyhow::Result<()>` (`head_bucket`). `get` maps `NoSuchKey` to `Ok(None)`.

- [ ] **Step 1: Write failing tests:** `settings_normalize_trims_and_strips_slash` (Review Focus 5: `"  https://cdn.x.com/ "` → `"https://cdn.x.com"`, `" key "` → `"key"`); `masked_hides_secret`; `load_missing_returns_default`; `save_then_load_roundtrip` (tempdir). `r2_live` (`#[ignore]`) reads `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_BUCKET` from the env: test_connection → put/get/delete `test/probe.txt` → get returns None after the delete.
- [ ] **Step 2: Run:** `cargo test --test settings`. Expected: FAIL.
- [ ] **Step 3: Implement** both modules.
- [ ] **Step 4: Run:** `cargo test --test settings`. Expected: PASS. `cargo test --test r2_live -- --ignored` is run in Task 14 with real credentials.
- [ ] **Step 5: Commit:** `✨ Add settings persistence and R2 storage`

---

### Task 10: Tauri commands and upload batching (`commands.rs`)

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/commands.rs` (tests the pure batch runner, not Tauri)

**Interfaces:**
- Consumes: Tasks 7–9
- Produces (TS names in Task 12 mirror these exactly):
  - `AppState { settings_path: PathBuf, settings: RwLock<Settings>, library: RwLock<Option<Arc<Library>>> }`. The library is (re)built on startup and on `save_settings` when `is_complete`.
  - Commands:
    - `get_settings() -> Settings` (masked)
    - `save_settings(s: Settings) -> Result<(), String>`: if `secret_access_key` starts with `"••••"`, keep the stored secret
    - `test_connection(s: Settings) -> Result<(), String>`: same secret-keeping rule
    - `list_tracks() -> Result<Vec<TrackSummary>, LibraryError>`
    - `get_track(id: String) -> Result<TrackRecord, LibraryError>`
    - `upload_files(paths: Vec<String>) -> Result<(), String>`: returns at once and runs the batch in the background
    - `update_track(id: String, edit: TagEdit, cover_path: Option<String>) -> Result<TrackSummary, LibraryError>`
    - `delete_tracks(ids: Vec<String>) -> Result<(), LibraryError>`
    - No configured library → `LibraryError::Validation("NOT_CONFIGURED")`.
  - Event `"upload-progress"`, payload `UploadProgress { path: String, fileName: String, stage: "queued"|"analysing"|"uploading"|"done"|"duplicate"|"error", errorCode: Option<String>, message: Option<String>, track: Option<TrackSummary> }`.
  - `pub async fn run_batch(paths: Vec<PathBuf>, library: Arc<Library>, emit: impl Fn(UploadProgress) + Send + Sync + 'static)`. It emits `queued` for all, then analyses with concurrency = `available_parallelism()` (a Semaphore around `process_with_timeout`). The `add` calls are serialised by the Library lock. Every file ends in exactly one terminal event (`done` / `duplicate` / `error`). A failure never stops the batch.

- [ ] **Step 1: Write failing test:** `batch_emits_one_terminal_event_per_file`: paths `[pink_128, pink_128 (again), fake, short, cover_small]` against `MemoryStorage` → terminal stages `{done, duplicate, error INVALID_FORMAT, error TOO_SHORT, done}` (matched by path, any order). The catalog has 2 tracks.
- [ ] **Step 2: Run:** `cargo test --test commands`. Expected: FAIL.
- [ ] **Step 3: Implement** `run_batch`, the commands and the state wiring, and register them in `lib.rs`. In `tauri.conf.json`, set a CSP with `img-src 'self' https: data:`.
- [ ] **Step 4: Run:** `cargo test` (all). Expected: PASS. `cargo clippy -- -D warnings` is clean.
- [ ] **Step 5: Commit:** `✨ Expose library commands and background upload batch`

---

### Task 11: UI design pass (`ui-ux-pro-max:design`)

**Files:**
- Create: `docs/design/README.md` (direction, palette, type, spacing, component specs), `src/styles/tokens.css` (CSS custom properties, light + dark), `docs/design/mockup.html` (static Library + Detail + Upload panel, using the tokens)

**Interfaces:**
- Produces: the token names that Tasks 12–13 use (`--color-*`, `--space-*`, `--radius-*`, `--font-*`), and the component specs for table row, quality badge, waveform, drop zone, dialog, form field, toast.

- [ ] **Step 1:** Invoke the `ui-ux-pro-max:design` skill (the user asked for it by name; pull in `ui-ux-pro-max:ui-ux-pro-max` / `design-system` if it routes there). Brief: a desktop admin tool for a hi-res music library; dense data table; audio-technical metadata (LUFS, dBTP, bandwidth, gapless) shown clearly; quality badges for full/high/medium/low; waveform; Vietnamese UI copy; light + dark.
- [ ] **Step 2:** Write the three files above. The mockup uses realistic data (Vietnamese titles, the 4 quality levels, a track with no cover).
- [ ] **Step 3: Checkpoint:** show the user the mockup (open it in the browser). **Wait for approval** before Task 12.
- [ ] **Step 4: Commit:** `🎨 Add UI design system and mockup`

---

### Task 12: Frontend shell: API layer, Library, Detail, Settings

**Files:**
- Create: `src/api.ts`, `src/api.mock.ts`, `src/types.ts`, `src/format.ts`, `src/format.test.ts`, `src/views/library.ts`, `src/views/detail.ts`, `src/views/settings.ts`, `src/styles/app.css`
- Modify: `index.html`, `src/main.ts`

**Interfaces:**
- Consumes: the commands, payloads and event from Task 10, and the tokens from Task 11
- Produces:
  - `types.ts`: `TrackSummary`, `TrackRecord`, `TagEdit`, `Settings`, `UploadProgress`, `AddOutcome`, `LibraryError`, mirroring the Rust camelCase fields
  - `api.ts`: `listTracks()`, `getTrack(id)`, `getSettings()`, `saveSettings(s)`, `testConnection(s)`, `uploadFiles(paths)`, `updateTrack(id, edit, coverPath?)`, `deleteTracks(ids)`, `onUploadProgress(cb) -> unlisten`, `pickFiles(): Promise<string[]>` (dialog plugin, filter `mp3`, multiple) and `pickImage()`. If `window.__TAURI_INTERNALS__` is absent, it delegates to `api.mock.ts` (in-memory fake tracks covering all 4 hints and a no-cover track).
  - `format.ts`: `qualityLabel(bitrate, hint)` per the spec's call ("MP3 {bitrate} · Full band" / "· High (~V0–V2)" / medium → "· nguồn ~128k" if bitrate ≥ 192 else "· Medium" / low → "· Nguồn kém"; a null hint → "MP3 {bitrate}"); `formatDuration(ms)` (`m:ss`, `h:mm:ss` ≥ 1 h); `formatSize(bytes)` (MB, 1 decimal); `formatLufs(x)` (1 decimal + " LUFS")

- [ ] **Step 1: Write failing tests** (`src/format.test.ts`): `qualityLabel(320,"medium") === "MP3 320 · nguồn ~128k"`, `qualityLabel(160,"medium") === "MP3 160 · Medium"`, `qualityLabel(320,"full") === "MP3 320 · Full band"`, `qualityLabel(128,null) === "MP3 128"`, `formatDuration(390000) === "6:30"`, `formatDuration(3_725_000) === "1:02:05"`, `formatSize(15_600_000) === "14.9 MB"`.
- [ ] **Step 2: Run:** `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** `format.ts`, `types.ts` and `api.ts`/`api.mock.ts`, then the views per the design:
  - Library: table, multi-select, loading / empty / error states (a `CatalogCorrupt` message tells the user that nothing will be written)
  - Detail: waveform drawn on `<canvas>` from 800 points, loudness / peak / LRA, bandwidth + hint, gapless numbers or "Không có thông tin gapless", technical fields
  - Settings: form + Test connection + Save; first run without complete settings → Settings
- [ ] **Step 4: Verify:** `npm test` PASS. `npm run build` OK. Run `npm run dev`, open it in Chrome (mock mode) and screenshot Library, Detail and Settings in light and dark. Compare against the Task 11 mockup.
- [ ] **Step 5: Commit:** `✨ Add library, detail and settings views`

---

### Task 13: Frontend: Upload panel, Edit dialog, Delete confirm

**Files:**
- Create: `src/views/upload.ts`, `src/views/edit-dialog.ts`, `src/views/confirm-dialog.ts`
- Modify: `src/main.ts`, `src/api.mock.ts` (simulate staged progress events)

**Interfaces:**
- Consumes: `api.ts`, `format.ts`, the Task 11 component specs

- [ ] **Step 1: Implement** the three views:
  - Upload: drop zone using Tauri's window drag-drop event (`getCurrentWebview().onDragDropEvent`, `.mp3` paths only) + "Chọn file" button → `uploadFiles`. The queue shows one row per file with stage, the error code mapped to Vietnamese text, and the duplicate state linking to the existing track. The library refreshes on each `done`.
  - Edit: the 8 tag fields, title required, "Đổi ảnh bìa" via `pickImage` → `updateTrack`.
  - Delete confirm: names the count and titles, calls `deleteTracks`, and is used for both row delete and bulk delete.
- [ ] **Step 2: Verify:** `npm run build` OK. In mock mode in Chrome: upload 3 fake files (see the stages progress, one duplicate, one error), edit a title, bulk-delete 2 tracks. Screenshot each.
- [ ] **Step 3: Commit:** `✨ Add upload queue, edit and delete dialogs`

---

### Task 14: End-to-end verification against real R2

**Files:**
- Create: `README.md` (setup: R2 bucket public access, settings fields, `npm run tauri dev`/`build`, regenerating fixtures)

- [ ] **Step 1:** Ask the user for a test bucket and its credentials (they enter them in Settings themselves). Run `cargo test --test r2_live -- --ignored` with the env vars. Expected: PASS.
- [ ] **Step 2: Perf:** generate a 4-min pink-noise 320k MP3 with ffmpeg (not committed) and time `process` in a release build (`cargo test --release --test pipeline perf_4min -- --ignored --nocapture`, add that ignored test). Expected: ≲ 4 s.
- [ ] **Step 3: Manual E2E** in `npm run tauri dev`, checking `curl -s <public-base>/catalog.json` after each step:
  1. Upload `cover_big.mp3`, `pink_v2.mp3`, `noxing_notag.mp3` → 3 tracks. `catalog.json` has no `waveform`, and `meta/<id>.json` has 800 values.
  2. `curl -sI -H "Range: bytes=0-1023" <audioUrl>` → `206`, `Content-Type: audio/mpeg`, the immutable Cache-Control, no `Content-Encoding`.
  3. Edit a title and swap a cover → the catalog changes, and the new `coverUrl` key differs and loads.
  4. Delete 2 tracks → they're gone from the catalog, and their `audioUrl` returns 404.
- [ ] **Step 4:** `npm run tauri build` produces an installer under `src-tauri/target/release/bundle/`.
- [ ] **Step 5: Commit:** `📝 Add README and live verification`
