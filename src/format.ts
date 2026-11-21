import type { QualityHint } from "./types";

export function qualityLabel(bitrate: number, hint: QualityHint | null): string {
  const base = `MP3 ${bitrate}`;
  switch (hint) {
    case "full":
      return `${base} · Full band`;
    case "high":
      return `${base} · High (~V0–V2)`;
    case "medium":
      return bitrate >= 192 ? `${base} · nguồn ~128k` : `${base} · Medium`;
    case "low":
      return `${base} · Nguồn kém`;
    default:
      return base;
  }
}

/** Number of lit bars (0..4) for a quality hint. */
export function qualityBars(hint: QualityHint | null): number {
  return hint === null ? 0 : { full: 4, high: 3, medium: 2, low: 1 }[hint];
}

export function formatDuration(ms: number): string {
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export function formatSize(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/** Bare number with one decimal, as used in the table column. */
export function lufsNumber(x: number): string {
  return x.toFixed(1);
}

export function formatLufs(x: number): string {
  return `${lufsNumber(x)} LUFS`;
}

/** One decimal with an explicit "+" above 0. */
export function signedNumber(x: number): string {
  const s = x.toFixed(1);
  return x > 0 ? `+${s}` : s;
}

export function formatTruePeak(x: number): string {
  return `${signedNumber(x)} dBTP`;
}

export function formatKhz(hz: number, decimals = 2): string {
  return `${(hz / 1000).toFixed(decimals)} kHz`;
}

/** Integer grouped with U+202F (narrow no-break space). */
export function formatCount(n: number): string {
  return Math.round(n).toString().replace(/\B(?=(\d{3})+(?!\d))/g, " ");
}

/** dd/MM/yyyy HH:mm, local time. */
export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getDate())}/${p(d.getMonth() + 1)}/${d.getFullYear()} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
