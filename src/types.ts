export type QualityHint = "full" | "high" | "medium" | "low";

export interface TrackSummary {
  id: string;
  sourceSha256: string;
  audioKey: string;
  coverKey: string | null;
  audioUrl: string;
  coverUrl: string | null;
  sizeBytes: number;
  codec: string;
  bitrate: number;
  vbr: boolean;
  sampleRate: number;
  channels: number;
  durationMs: number;
  title: string;
  artist: string | null;
  album: string | null;
  albumArtist: string | null;
  date: string | null;
  genre: string | null;
  track: string | null;
  disc: string | null;
  integratedLufs: number;
  truePeakDbtp: number;
  lra: number;
  effectiveBandwidthHz: number | null;
  qualityHint: QualityHint | null;
  trimStart: number | null;
  trimEnd: number | null;
  totalSamples: number | null;
  analysisVersion: number;
  createdAt: string;
  updatedAt: string;
}

export interface TrackRecord extends TrackSummary {
  /** 800 peaks, 0..1 */
  waveform: number[];
}

export interface TagEdit {
  title: string;
  artist: string | null;
  album: string | null;
  albumArtist: string | null;
  date: string | null;
  genre: string | null;
  track: string | null;
  disc: string | null;
}

export interface Settings {
  accountId: string;
  accessKeyId: string;
  secretAccessKey: string;
  bucket: string;
  publicBaseUrl: string;
}

export type UploadStage =
  | "queued"
  | "analysing"
  | "uploading"
  | "done"
  | "duplicate"
  | "error";

export interface UploadProgress {
  path: string;
  fileName: string;
  stage: UploadStage;
  errorCode: string | null;
  message: string | null;
  track: TrackSummary | null;
}

export interface AddOutcome {
  kind: "added" | "duplicate";
  track: TrackSummary;
}

export interface LibraryError {
  code: "CATALOG_CORRUPT" | "NOT_FOUND" | "VALIDATION" | "STORAGE";
  message: string;
}

export const NOT_CONFIGURED = "NOT_CONFIGURED";

export function isLibraryError(e: unknown): e is LibraryError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

export function isNotConfigured(e: unknown): boolean {
  return isLibraryError(e) && e.message === NOT_CONFIGURED;
}
