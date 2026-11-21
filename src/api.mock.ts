// In-memory fake backend, used when the app runs in a plain browser (no Tauri).
// Query params: ?unconfigured (first run), ?corrupt (CATALOG_CORRUPT), ?empty (no tracks).
import type {
  LibraryError,
  QualityHint,
  Settings,
  TagEdit,
  TrackRecord,
  TrackSummary,
  UploadProgress,
} from "./types";

const params = new URLSearchParams(globalThis.location?.search ?? "");

let settings: Settings = params.has("unconfigured")
  ? { accountId: "", accessKeyId: "", secretAccessKey: "", bucket: "", publicBaseUrl: "" }
  : {
      accountId: "4f1c2a9e7b3d4c58a1e6f0b2d9c37a64",
      accessKeyId: "8d2e61f0a4c94b7e93d15a2c",
      secretAccessKey: "••••a1b2",
      bucket: "hires-library",
      publicBaseUrl: "https://pub-7c3e91d4a2.r2.dev",
    };

const isComplete = () => Object.values(settings).every((v) => v.trim().length > 0);

function libraryError(code: LibraryError["code"], message: string): LibraryError {
  return { code, message };
}

function waveform(seed: number): number[] {
  const out: number[] = [];
  for (let i = 0; i < 800; i++) {
    const t = i / 800;
    const env = Math.min(1, t * 12) * Math.min(1, (1 - t) * 12);
    const body = 0.45 + 0.3 * Math.sin(t * 9 + seed) + 0.15 * Math.sin(t * 41 + seed * 2);
    const noise = 0.12 * Math.sin(i * 12.9898 + seed * 78.233);
    out.push(Math.max(0.02, Math.min(1, (body + noise) * (0.35 + 0.65 * env))));
  }
  return out;
}

interface Seed {
  title: string;
  artist: string | null;
  album: string | null;
  albumArtist: string | null;
  date: string | null;
  genre: string | null;
  track: string | null;
  duration: string;
  bitrate: number;
  vbr: boolean;
  hint: QualityHint | null;
  bwHz: number | null;
  lufs: number;
  peak: number;
  lra: number;
  gapless: boolean;
  cover: boolean;
  sampleRate?: number;
}

const seeds: Seed[] = [
  { title: "Đêm Sài Gòn", artist: "Ngọc Lan Vy", album: "Phố Đêm", albumArtist: "Ngọc Lan Vy", date: "2023", genre: "Pop ballad", track: "1", duration: "4:12", bitrate: 320, vbr: false, hint: "full", bwHz: 20300, lufs: -9.8, peak: -0.4, lra: 5.2, gapless: true, cover: true },
  { title: "Mưa Trên Phố Huế", artist: "Trần Minh Khang", album: "Cố Đô", albumArtist: "Trần Minh Khang", date: "2022", genre: "Nhạc trữ tình", track: "4", duration: "5:03", bitrate: 245, vbr: true, hint: "high", bwHz: 19100, lufs: -11.4, peak: -1.1, lra: 6.8, gapless: true, cover: true },
  { title: "Gió Lộng Tây Nguyên", artist: "Hà My & Đồi Thông", album: "Cao Nguyên", albumArtist: "Hà My", date: "2024", genre: "Dân gian đương đại", track: "3", duration: "3:47", bitrate: 320, vbr: false, hint: "medium", bwHz: 17150, lufs: -8.1, peak: 0.3, lra: 4.6, gapless: true, cover: true },
  { title: "Bên Kia Sông", artist: "Lê Quang Huy", album: "Mùa Nước Nổi", albumArtist: null, date: "2019", genre: "Folk", track: "2", duration: "4:31", bitrate: 128, vbr: false, hint: "low", bwHz: 14800, lufs: -12.6, peak: -2.3, lra: 7.4, gapless: false, cover: true },
  { title: "Thu Cạn", artist: "Phạm Thu Trang", album: "Lá Vàng", albumArtist: "Phạm Thu Trang", date: "2021", genre: "Indie", track: "6", duration: "3:58", bitrate: 320, vbr: false, hint: "full", bwHz: 20500, lufs: -10.2, peak: -0.8, lra: 8.1, gapless: true, cover: false },
  { title: "Phố Cũ Mùa Đông", artist: "Đoàn Bảo Nam", album: "Một Thời Để Nhớ", albumArtist: "Đoàn Bảo Nam", date: "2020", genre: "Nhạc vàng", track: "7", duration: "6:30", bitrate: 190, vbr: true, hint: "high", bwHz: 18700, lufs: -13.1, peak: -1.9, lra: 9.3, gapless: true, cover: true },
  { title: "Ru Con Miền Tây", artist: "Nguyễn Kim Ngân", album: "Ca Dao Mẹ", albumArtist: "Nguyễn Kim Ngân", date: "2018", genre: "Dân ca", track: "1", duration: "2:54", bitrate: 160, vbr: false, hint: "medium", bwHz: 16900, lufs: -14.7, peak: -3.2, lra: 3.9, gapless: false, cover: true },
  { title: "Nắng Ấm Xa Dần", artist: "Vũ Đăng Khoa", album: "Ngày Xưa Ơi", albumArtist: "Vũ Đăng Khoa", date: "2017", genre: "Pop", track: "5", duration: "4:45", bitrate: 320, vbr: false, hint: "full", bwHz: 20800, lufs: -7.9, peak: -0.2, lra: 4.1, gapless: true, cover: true, sampleRate: 48000 },
  { title: "Bài Không Nhãn", artist: null, album: null, albumArtist: null, date: null, genre: null, track: null, duration: "3:21", bitrate: 256, vbr: true, hint: null, bwHz: null, lufs: -10.9, peak: -1.5, lra: 6.0, gapless: false, cover: false },
];

function hex(n: number, len: number): string {
  let s = "";
  for (let i = 0; s.length < len; i++) {
    s += Math.floor(Math.abs(Math.sin((n + 1) * (i + 3) * 9301.7)) * 16).toString(16);
  }
  return s;
}

function makeRecord(s: Seed, i: number): TrackRecord {
  const [m, sec] = s.duration.split(":").map(Number);
  const durationMs = (m * 60 + sec) * 1000;
  const id = hex(i, 64);
  const sizeBytes = Math.round((s.bitrate * 1000 * (durationMs / 1000)) / 8);
  const hue = (i * 47 + 22) % 360;
  const cover = s.cover
    ? `data:image/svg+xml;utf8,${encodeURIComponent(
        `<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue} 62% 52%)"/><stop offset="1" stop-color="hsl(${(hue + 48) % 360} 70% 24%)"/></linearGradient></defs><rect width="96" height="96" fill="url(#g)"/></svg>`,
      )}`
    : null;
  const sampleRate = s.sampleRate ?? 44100;
  const total = Math.round((durationMs / 1000) * sampleRate);
  const created = new Date(Date.UTC(2026, 10, 21, 3, 42 + i * 3)).toISOString();
  return {
    id,
    sourceSha256: hex(i + 100, 64),
    audioKey: `audio/${id}.mp3`,
    coverKey: s.cover ? `cover/${id}.jpg` : null,
    audioUrl: `https://pub-7c3e91d4a2.r2.dev/audio/${id}.mp3`,
    coverUrl: cover,
    sizeBytes,
    codec: "mp3",
    bitrate: s.bitrate,
    vbr: s.vbr,
    sampleRate,
    channels: i === 6 ? 1 : 2,
    durationMs,
    title: s.title,
    artist: s.artist,
    album: s.album,
    albumArtist: s.albumArtist,
    date: s.date,
    genre: s.genre,
    track: s.track,
    disc: s.track ? "1" : null,
    integratedLufs: s.lufs,
    truePeakDbtp: s.peak,
    lra: s.lra,
    effectiveBandwidthHz: s.bwHz,
    qualityHint: s.hint,
    trimStart: s.gapless ? 1105 : null,
    trimEnd: s.gapless ? 695 : null,
    totalSamples: s.gapless ? total : null,
    analysisVersion: 1,
    createdAt: created,
    updatedAt: created,
    waveform: waveform(i + 1),
  };
}

let tracks: TrackRecord[] = params.has("empty") ? [] : seeds.map(makeRecord);

function summary(t: TrackRecord): TrackSummary {
  const { waveform: _w, ...s } = t;
  return s;
}

const delay = <T>(v: T, ms = 250) => new Promise<T>((r) => setTimeout(() => r(v), ms));

function requireLibrary() {
  if (!isComplete()) throw libraryError("VALIDATION", "NOT_CONFIGURED");
  if (params.has("corrupt"))
    throw libraryError("CATALOG_CORRUPT", "catalog.json is not valid JSON");
}

type Listener = (p: UploadProgress) => void;
const listeners = new Set<Listener>();
const emit = (p: UploadProgress) => listeners.forEach((l) => l(p));

export const mockApi = {
  async listTracks(): Promise<TrackSummary[]> {
    await delay(null, 400);
    requireLibrary();
    return tracks.map(summary);
  },
  async getTrack(id: string): Promise<TrackRecord> {
    requireLibrary();
    const t = tracks.find((x) => x.id === id);
    if (!t) throw libraryError("NOT_FOUND", "track not found");
    return delay(t, 120);
  },
  async getSettings(): Promise<Settings> {
    return { ...settings };
  },
  async saveSettings(s: Settings): Promise<void> {
    settings = { ...s };
    await delay(null);
  },
  async testConnection(s: Settings): Promise<void> {
    await delay(null, 700);
    if (!Object.values(s).every((v) => v.trim()))
      throw "Thiếu thông tin: điền đủ các trường trước khi kiểm tra.";
  },
  async uploadFiles(paths: string[]): Promise<void> {
    // Demo script: 2nd file of a batch is a duplicate of an existing track, 3rd fails to parse.
    for (const [index, path] of paths.entries()) {
      const fileName = path.split(/[\\/]/).pop() ?? path;
      const base = { path, fileName, errorCode: null, message: null, track: null };
      emit({ ...base, stage: "queued" });
      setTimeout(() => emit({ ...base, stage: "analysing" }), 300);
      setTimeout(() => emit({ ...base, stage: "uploading" }), 900);
      if (index % 5 === 1 && tracks.length > 0) {
        setTimeout(() => emit({ ...base, stage: "duplicate", track: summary(tracks[0]) }), 1100);
        continue;
      }
      if (index % 5 === 2) {
        setTimeout(
          () => emit({ ...base, stage: "error", errorCode: "INVALID_FORMAT", message: "not an mp3" }),
          1200,
        );
        continue;
      }
      setTimeout(() => {
        const t = makeRecord(seeds[tracks.length % seeds.length], tracks.length + 20);
        t.title = fileName.replace(/\.mp3$/i, "");
        tracks = [...tracks, t];
        emit({ ...base, stage: "done", track: summary(t) });
      }, 1500);
    }
  },
  async updateTrack(
    id: string,
    edit: TagEdit,
    _coverPath?: string | null,
  ): Promise<TrackSummary> {
    requireLibrary();
    const i = tracks.findIndex((x) => x.id === id);
    if (i < 0) throw libraryError("NOT_FOUND", "track not found");
    tracks[i] = { ...tracks[i], ...edit, updatedAt: new Date().toISOString() };
    return delay(summary(tracks[i]));
  },
  async deleteTracks(ids: string[]): Promise<void> {
    requireLibrary();
    tracks = tracks.filter((t) => !ids.includes(t.id));
    await delay(null);
  },
  onUploadProgress(cb: Listener): () => void {
    listeners.add(cb);
    return () => listeners.delete(cb);
  },
  async pickFiles(): Promise<string[]> {
    return [
      "C:/Music/em-oi-ha-noi-pho.mp3",
      "C:/Music/Dem Sai Gon (master).mp3",
      "C:/Music/intro-loop.mp3",
    ];
  },
  async pickImage(): Promise<string | null> {
    return null;
  },
};
