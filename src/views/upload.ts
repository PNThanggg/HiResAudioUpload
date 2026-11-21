import { getCurrentWebview } from "@tauri-apps/api/webview";
import { onUploadProgress, pickFiles, uploadFiles } from "../api";
import { esc, icon, qs, toast } from "../dom";
import {
  applyProgress,
  clearFinished,
  errorText,
  summarize,
  type QueueItem,
} from "../upload-queue";
import { isLibraryError } from "../types";
import { qualityBadge } from "./library";

export interface UploadHooks {
  /** Open the upload panel (a drop landed anywhere on the window). */
  openPanel(): void;
  /** A file finished uploading: the library should reload. */
  onDone(): void;
}

export interface UploadPanelCallbacks {
  onClose(): void;
  /** "Xem" / duplicate link: open that track's detail. */
  onOpenTrack(id: string): void;
}

export interface UploadHandle {
  dispose(): void;
}

const inTauri = "__TAURI_INTERNALS__" in window;

// Module-level so the queue survives closing and reopening the panel.
let queue: QueueItem[] = [];
/** Number of acceptable files being dragged over the window, or null when not dragging. */
let dragCount: number | null = null;
const subscribers = new Set<() => void>();
let started = false;

const notify = () => subscribers.forEach((fn) => fn());
const isMp3 = (p: string) => p.toLowerCase().endsWith(".mp3");

/** Starts uploads for `paths` (already filtered to .mp3 by the callers). */
export async function startUpload(paths: string[]): Promise<void> {
  if (paths.length === 0) return;
  try {
    await uploadFiles(paths);
  } catch (e) {
    const code = isLibraryError(e) ? e.code : null;
    const message = isLibraryError(e) ? e.message : String(e);
    toast("error", "Không bắt đầu được tải lên", errorText(code, message));
  }
}

function acceptPaths(paths: string[]): string[] {
  const ok = paths.filter(isMp3);
  const skipped = paths.length - ok.length;
  if (skipped > 0) toast("info", `Bỏ qua ${skipped} file không phải .mp3`);
  return ok;
}

/** Registers the progress listener and the window drag-drop handler. Call once at startup. */
export function initUpload(hooks: UploadHooks): void {
  if (started) return;
  started = true;

  onUploadProgress((ev) => {
    queue = applyProgress(queue, ev);
    notify();
    if (ev.stage === "done") hooks.onDone();
  });

  if (!inTauri) return; // browser mock: the "Chọn file…" button is the entry point
  void getCurrentWebview().onDragDropEvent((event) => {
    const p = event.payload;
    if (p.type === "enter") {
      dragCount = p.paths.filter(isMp3).length;
      hooks.openPanel();
      notify();
    } else if (p.type === "leave") {
      dragCount = null;
      notify();
    } else if (p.type === "drop") {
      dragCount = null;
      hooks.openPanel();
      notify();
      void startUpload(acceptPaths(p.paths));
    }
  });
}

function dropzone(): string {
  const dragging = dragCount !== null;
  const compact = queue.length > 0 && !dragging;
  const title = dragging
    ? dragCount && dragCount > 0
      ? `Thả để tải lên ${dragCount} file`
      : "Chỉ nhận file .mp3"
    : queue.length > 0
      ? "Kéo thả thêm file MP3"
      : "Kéo thả file MP3 vào đây";
  const hint = dragging ? "Chỉ nhận file .mp3" : "Tối đa 200 MB · dài 1 giây đến 2 giờ";
  const pick = `<button class="btn btn--secondary btn--sm" type="button" data-act="pick">${icon("file-audio")}Chọn file…</button>`;
  const cls = `dropzone${compact ? " dropzone--compact" : ""}${dragging ? " is-dragover" : ""}`;
  if (compact) {
    return `<div class="${cls}">
      <span class="dropzone-icon">${icon("upload")}</span>
      <div class="dropzone-text"><p class="dropzone-title">${title}</p><p class="dropzone-hint">${hint}</p></div>
      ${pick}
    </div>`;
  }
  return `<div class="${cls}">
    <span class="dropzone-icon">${icon("upload", "icon--lg")}</span>
    <p class="dropzone-title">${title}</p>
    ${dragging ? "" : '<p class="dropzone-hint">hoặc</p>'}
    ${dragging ? "" : pick}
    <p class="dropzone-hint">${hint}</p>
  </div>`;
}

const STAGE_ICON: Record<QueueItem["stage"], string> = {
  queued: icon("clock"),
  analysing: icon("loader", "spin"),
  uploading: icon("upload"),
  done: icon("check-circle"),
  duplicate: icon("copy"),
  error: icon("alert-circle"),
};

const STAGE_LABEL: Record<QueueItem["stage"], string> = {
  queued: "Đang chờ",
  analysing: "Đang phân tích",
  uploading: "Đang tải lên",
  done: "Hoàn tất",
  duplicate: "Trùng lặp",
  error: "Lỗi",
};

function queueItem(it: QueueItem): string {
  const chip =
    it.stage === "error" && it.errorCode
      ? `Lỗi · <span class="mono">${esc(it.errorCode)}</span>`
      : STAGE_LABEL[it.stage];
  let meta = "";
  let bar = "";
  if (it.stage === "done" && it.track) {
    meta = `${qualityBadge(it.track.bitrate, it.track.qualityHint, "sm")}<a class="link" href="#" data-open="${esc(it.track.id)}">Xem</a>`;
  } else if (it.stage === "duplicate" && it.track) {
    meta = `<span>Đã có trong thư viện: <a class="link" href="#" data-open="${esc(it.track.id)}">${esc(it.track.title)}</a></span>`;
  } else if (it.stage === "error") {
    meta = `<span class="qitem-error">${esc(errorText(it.errorCode, it.message))}</span>`;
  } else if (it.stage === "analysing" || it.stage === "uploading") {
    // No byte-level progress is reported, so both stages use the indeterminate bar.
    bar = `<div class="progress progress--indeterminate" role="progressbar" aria-label="${STAGE_LABEL[it.stage]}"><span></span></div>`;
  }
  return `<li class="qitem qitem--${it.stage}">
    <span class="qitem-icon">${STAGE_ICON[it.stage]}</span>
    <span class="qitem-name" title="${esc(it.fileName)}">${esc(it.fileName)}</span>
    <span class="stage stage--${it.stage}">${chip}</span>
    ${meta ? `<span class="qitem-meta">${meta}</span>` : ""}
    ${bar}
  </li>`;
}

function queueSection(): string {
  if (queue.length === 0) return "";
  const s = summarize(queue);
  const parts = [`${s.total} file`];
  if (s.done) parts.push(`${s.done} xong`);
  if (s.duplicate) parts.push(`${s.duplicate} trùng`);
  if (s.error) parts.push(`${s.error} lỗi`);
  if (s.active) parts.push(`${s.active} đang xử lý`);
  return `<section>
    <div class="section-row">
      <h4 class="section-label">Hàng đợi</h4>
      <button class="btn btn--ghost btn--xs" type="button" data-act="clear"${s.total === s.active ? " disabled" : ""}>Xoá mục đã xong</button>
    </div>
    <p class="queue-summary num" role="status">${parts.join(" · ")}</p>
    <ul class="queue">${queue.map(queueItem).join("")}</ul>
  </section>`;
}

/** Renders the upload panel into the inspector slot. */
export function mountUpload(slot: HTMLElement, cb: UploadPanelCallbacks): UploadHandle {
  slot.innerHTML = `<div class="inspector-head"><h2 class="inspector-title">Tải lên</h2><button class="btn btn--ghost btn--icon" type="button" data-act="close" aria-label="Đóng bảng tải lên">${icon("x")}</button></div>
    <div class="inspector-body"><div data-slot="dropzone"></div><div data-slot="queue"></div></div>`;
  const zone = qs<HTMLElement>(slot, '[data-slot="dropzone"]');
  const list = qs<HTMLElement>(slot, '[data-slot="queue"]');

  let zoneHtml = "";
  const render = () => {
    const nextZone = dropzone();
    // Leave the dropzone alone when unchanged so focus on "Chọn file…" survives queue updates.
    if (nextZone !== zoneHtml) {
      zoneHtml = nextZone;
      zone.innerHTML = nextZone;
    }
    list.innerHTML = queueSection();
  };
  render();
  subscribers.add(render);

  const onClick = async (ev: Event) => {
    const target = ev.target as HTMLElement;
    const open = target.closest<HTMLElement>("[data-open]");
    if (open) {
      ev.preventDefault();
      cb.onOpenTrack(open.dataset.open ?? "");
      return;
    }
    const act = target.closest<HTMLElement>("[data-act]")?.dataset.act;
    if (act === "close") cb.onClose();
    else if (act === "clear") {
      queue = clearFinished(queue);
      notify();
    } else if (act === "pick") {
      await startUpload(acceptPaths(await pickFiles()));
    }
  };
  slot.addEventListener("click", onClick);

  return {
    dispose() {
      subscribers.delete(render);
      slot.removeEventListener("click", onClick);
    },
  };
}
