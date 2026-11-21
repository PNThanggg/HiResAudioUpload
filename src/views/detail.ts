import { getTrack } from "../api";
import { esc, icon, qs, toast } from "../dom";
import {
  formatCount,
  formatDateTime,
  formatDuration,
  formatKhz,
  formatSize,
  lufsNumber,
  signedNumber,
} from "../format";
import type { QualityHint, TrackRecord } from "../types";
import { isLibraryError } from "../types";
import { coverCell, qualityBadge } from "./library";

export interface DetailCallbacks {
  onClose(): void;
  /** Edit dialog (wired in Task 13). */
  onEdit(track: TrackRecord): void;
  /** Delete confirm (wired in Task 13). */
  onDelete(track: TrackRecord): void;
}

export interface DetailHandle {
  dispose(): void;
}

/** LC §5.2 bandwidth thresholds, in Hz. */
const THRESHOLDS = [15500, 18000, 19500];
const SCALE_MIN = 10000;

function hintText(hint: QualityHint | null, bitrate: number, bwHz: number | null): string {
  switch (hint) {
    case "full":
      return "Nội dung trải tới gần Nyquist: đúng chất lượng bitrate ghi.";
    case "high":
      return "Mức cắt kiểu VBR V0–V2.";
    case "medium":
      return bitrate >= 192
        ? `File ghi ${bitrate} kbps nhưng nội dung dừng ở ~${Math.round((bwHz ?? 0) / 1000)} kHz: nhiều khả năng là bản 128 kbps được encode lại.`
        : "Mức cắt kiểu 128–160 kbps.";
    case "low":
      return "Băng thông thấp: nguồn chất lượng kém.";
    default:
      return "Chưa có đánh giá chất lượng cho bài này.";
  }
}

function bandwidthMeter(t: TrackRecord): string {
  const bw = t.effectiveBandwidthHz;
  if (bw === null) return "";
  const nyquist = t.sampleRate / 2;
  const span = nyquist - SCALE_MIN;
  const pos = (hz: number) => Math.max(0, Math.min(100, ((hz - SCALE_MIN) / span) * 100));
  const [p1, p2, p3] = THRESHOLDS.map(pos);
  const at = pos(bw);
  const nyq = `${parseFloat((nyquist / 1000).toFixed(2))}k`;
  const label = `Băng thông thực ${formatKhz(bw)}, mức ${t.qualityHint ?? "không rõ"}`;
  return `<div class="bandwidth" role="img" aria-label="${esc(label)}">
    <div class="bandwidth-head"><span class="bandwidth-label">Băng thông thực</span><span class="bandwidth-value num">${formatKhz(bw)}</span></div>
    <div class="bw-track">
      <span class="bw-zone bw-zone--low" style="width:${p1}%"></span><span class="bw-zone bw-zone--medium" style="width:${p2 - p1}%"></span><span class="bw-zone bw-zone--high" style="width:${p3 - p2}%"></span><span class="bw-zone bw-zone--full" style="flex:1"></span>
      <span class="bw-fill bw-fill--${t.qualityHint ?? "none"}" style="width:${at}%"></span>
      <span class="bw-needle" style="left:${at}%"></span>
    </div>
    <div class="bw-scale num" aria-hidden="true"><span style="left:0">10k</span><span style="left:${p1}%">15.5k</span><span style="left:${p2}%">18k</span><span style="left:${p3}%">19.5k</span><span style="left:100%">${nyq}</span></div>
  </div>`;
}

function waveformTicks(durationMs: number): string {
  const total = durationMs / 1000;
  const step = total > 480 ? 120 : 60;
  const marks: number[] = [];
  for (let s = 0; s < total; s += step) marks.push(s);
  // Drop a minute mark that would collide with the end label.
  if (marks.length > 1 && total - marks[marks.length - 1] < total * 0.06) marks.pop();
  marks.push(total);
  return marks
    .map((s) => `<span style="left:${((s / total) * 100).toFixed(1)}%">${formatDuration(s * 1000)}</span>`)
    .join("");
}

function trackDisc(t: TrackRecord): string {
  if (!t.track && !t.disc) return "—";
  return `${t.track ?? "—"} / ${t.disc ?? "—"}`;
}

function channelsLabel(n: number): string {
  return n === 1 ? "Mono" : n === 2 ? "Stereo" : `${n} kênh`;
}

function render(t: TrackRecord): string {
  const meta = [t.album, t.date, t.genre].filter(Boolean).map(esc).join(" · ");
  const warn = t.truePeakDbtp > 0;
  const gapless =
    t.trimStart === null && t.trimEnd === null && t.totalSamples === null
      ? `<p class="hint">${icon("info")}<span>Không có thông tin gapless</span></p>`
      : `<dl class="stats">
          <div><dt>Bỏ đầu</dt><dd class="num">${t.trimStart === null ? "—" : `${formatCount(t.trimStart)} <small>mẫu</small>`}</dd></div>
          <div><dt>Bỏ cuối</dt><dd class="num">${t.trimEnd === null ? "—" : `${formatCount(t.trimEnd)} <small>mẫu</small>`}</dd></div>
          <div><dt>Tổng</dt><dd class="num">${t.totalSamples === null ? "—" : `${formatCount(t.totalSamples)} <small>mẫu</small>`}</dd></div>
        </dl>`;
  const bwMeter = bandwidthMeter(t);
  return `<div class="inspector-head">
    <h2 class="inspector-title">Chi tiết</h2>
    <button class="btn btn--ghost btn--icon" type="button" data-act="close" aria-label="Đóng chi tiết">${icon("x")}</button>
  </div>
  <div class="inspector-body">
    <div class="detail-hero">
      ${coverCell(t, "cover cover--detail", t.album ? `Ảnh bìa ${t.album}` : "Ảnh bìa")}
      <div class="detail-id">
        <h3 class="detail-title">${esc(t.title)}</h3>
        <p class="detail-sub">${esc(t.artist ?? "Không rõ nghệ sĩ")}</p>
        ${meta ? `<p class="detail-sub">${meta}</p>` : ""}
        <div class="detail-actions">
          <button class="btn btn--secondary btn--sm" type="button" data-act="edit">${icon("pencil")}Sửa</button>
          <button class="btn btn--danger-ghost btn--sm" type="button" data-act="delete">${icon("trash")}Xoá</button>
        </div>
      </div>
    </div>

    <section>
      <div class="section-row"><h4 class="section-label">Chất lượng</h4>${qualityBadge(t.bitrate, t.qualityHint, "lg")}</div>
      ${bwMeter}
      <p class="hint">${icon("info")}<span>${esc(hintText(t.qualityHint, t.bitrate, t.effectiveBandwidthHz))}</span></p>
    </section>

    <section>
      <h4 class="section-label">Waveform</h4>
      <figure class="waveform">
        <canvas role="img" aria-label="Waveform của ${esc(t.title)}"></canvas>
        <div class="waveform-ticks num" aria-hidden="true">${waveformTicks(t.durationMs)}</div>
      </figure>
    </section>

    <section>
      <h4 class="section-label">Độ lớn &amp; đỉnh</h4>
      <div class="metrics">
        <div class="metric"><span class="metric-label">Integrated</span><span class="metric-value num">${lufsNumber(t.integratedLufs)}<small>LUFS</small></span></div>
        <div class="metric${warn ? " metric--warn" : ""}"><span class="metric-label">True peak</span><span class="metric-value num">${signedNumber(t.truePeakDbtp)}<small>dBTP</small></span>${warn ? `<span class="metric-note">${icon("alert-triangle")}Vượt 0 dBTP</span>` : ""}</div>
        <div class="metric"><span class="metric-label">LRA</span><span class="metric-value num">${lufsNumber(t.lra)}<small>LU</small></span></div>
      </div>
    </section>

    <section>
      <h4 class="section-label">Gapless</h4>
      ${gapless}
    </section>

    <section>
      <h4 class="section-label">Kỹ thuật</h4>
      <dl class="kv">
        <dt>Định dạng</dt><dd>MP3 · ${t.vbr ? "VBR" : "CBR"} <span class="num">${t.bitrate}</span> kbps</dd>
        <dt>Tần số mẫu</dt><dd class="num">${t.sampleRate / 1000} kHz</dd>
        <dt>Kênh</dt><dd>${channelsLabel(t.channels)}</dd>
        <dt>Thời lượng</dt><dd class="num">${formatDuration(t.durationMs)}</dd>
        <dt>Dung lượng</dt><dd class="num">${formatSize(t.sizeBytes)}</dd>
        <dt>Track / Disc</dt><dd class="num">${esc(trackDisc(t))}</dd>
        <dt>Nghệ sĩ album</dt><dd>${esc(t.albumArtist ?? "—")}</dd>
        <dt>ID</dt><dd class="kv-id"><span class="mono truncate" title="${esc(t.id)}">${esc(t.id)}</span><button class="btn btn--ghost btn--icon btn--xs" type="button" data-act="copy" aria-label="Sao chép ID">${icon("copy")}</button></dd>
        <dt>Tạo lúc</dt><dd class="num">${formatDateTime(t.createdAt)}</dd>
        <dt>Cập nhật</dt><dd class="num">${formatDateTime(t.updatedAt)}</dd>
      </dl>
    </section>
  </div>`;
}

/** Draws mirrored peaks on the canvas using theme colors read from CSS tokens. */
function drawWaveform(canvas: HTMLCanvasElement, peaks: number[]): void {
  const dpr = window.devicePixelRatio || 1;
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  if (width === 0 || height === 0) return;
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const css = getComputedStyle(canvas);
  const bars = css.getPropertyValue("--color-waveform").trim();
  const base = css.getPropertyValue("--color-waveform-baseline").trim();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, width, height);
  const mid = height / 2;
  ctx.fillStyle = base;
  ctx.fillRect(0, Math.floor(mid), width, 1);
  ctx.fillStyle = bars;
  const n = peaks.length;
  const barW = width / n;
  for (let i = 0; i < n; i++) {
    const half = Math.max(0.5, Math.max(0, Math.min(1, peaks[i])) * (mid - 2));
    ctx.fillRect(i * barW, mid - half, Math.max(barW - 0.25, 0.5), half * 2);
  }
}

/** Loads and renders a track into the inspector slot. */
export function mountDetail(
  slot: HTMLElement,
  id: string,
  cb: DetailCallbacks,
): DetailHandle {
  let disposed = false;
  const cleanups: Array<() => void> = [];

  slot.innerHTML = `<div class="inspector-head"><h2 class="inspector-title">Chi tiết</h2><button class="btn btn--ghost btn--icon" type="button" data-act="close" aria-label="Đóng chi tiết">${icon("x")}</button></div>
    <div class="inspector-body" aria-busy="true"><div class="skeleton-row" style="padding:0"><span class="skeleton skeleton--cover" style="width:var(--size-cover-detail);height:var(--size-cover-detail)"></span><span class="skeleton-lines"><span class="skeleton" style="width:70%"></span><span class="skeleton" style="width:45%"></span></span></div></div>`;
  const onClose = (ev: Event) => {
    if ((ev.target as HTMLElement).closest('[data-act="close"]')) cb.onClose();
  };
  slot.addEventListener("click", onClose);
  cleanups.push(() => slot.removeEventListener("click", onClose));

  void (async () => {
    let t: TrackRecord;
    try {
      t = await getTrack(id);
    } catch (e) {
      if (disposed) return;
      const msg = isLibraryError(e) ? e.message : String(e);
      qs<HTMLElement>(slot, ".inspector-body").innerHTML = `<div class="banner banner--danger" role="alert">${icon("alert-triangle")}<div><p class="banner-title">Không tải được chi tiết</p><p class="banner-msg">${esc(msg)}</p></div></div>`;
      return;
    }
    if (disposed) return;
    slot.innerHTML = render(t);

    const canvas = qs<HTMLCanvasElement>(slot, "canvas");
    const redraw = () => drawWaveform(canvas, t.waveform);
    redraw();
    const ro = new ResizeObserver(redraw);
    ro.observe(canvas);
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", redraw);
    const mo = new MutationObserver(redraw);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    cleanups.push(() => {
      ro.disconnect();
      mq.removeEventListener("change", redraw);
      mo.disconnect();
    });

    const onAct = (ev: Event) => {
      const act = (ev.target as HTMLElement).closest<HTMLElement>("[data-act]")?.dataset.act;
      if (act === "edit") cb.onEdit(t);
      else if (act === "delete") cb.onDelete(t);
      else if (act === "copy") {
        navigator.clipboard
          .writeText(t.id)
          .then(() => toast("info", "Đã sao chép ID"))
          .catch(() => toast("error", "Không sao chép được ID"));
      }
    };
    slot.addEventListener("click", onAct);
    cleanups.push(() => slot.removeEventListener("click", onAct));
  })();

  return {
    dispose() {
      disposed = true;
      for (const c of cleanups) c();
    },
  };
}

