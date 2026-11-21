import { listTracks } from "../api";
import { esc, icon, qs } from "../dom";
import {
  formatDuration,
  formatSize,
  lufsNumber,
  qualityBars,
  qualityLabel,
} from "../format";
import type { QualityHint, TrackSummary } from "../types";
import { isLibraryError, isNotConfigured } from "../types";

export type LibraryState =
  | { kind: "ok"; tracks: TrackSummary[] }
  | { kind: "not-configured" }
  | { kind: "corrupt"; message: string }
  | { kind: "error"; message: string };

export interface LibraryCallbacks {
  /** A row was clicked / Enter pressed: show its detail. */
  onOpen(id: string): void;
  /** Bulk or keyboard delete (wired in Task 13). */
  onDeleteRequest(ids: string[]): void;
  /** Empty-state "Tải lên" button (wired in Task 13). */
  onUploadRequest(): void;
  /** Called after every load attempt, for the top bar. */
  onState(state: LibraryState): void;
}

export interface LibraryHandle {
  reload(): Promise<void>;
  setActive(id: string | null): void;
  selectedIds(): string[];
  /** Returns true when something was selected. */
  clearSelection(): boolean;
  tracks(): TrackSummary[];
}

/** Quality badge markup, shared with the detail view. */
export function qualityBadge(
  bitrate: number,
  hint: QualityHint | null,
  size: "" | "sm" | "lg" = "",
  title = "",
): string {
  const bars = qualityBars(hint);
  const lit = [0, 1, 2, 3].map((i) => `<i${i < bars ? ' class="on"' : ""}></i>`).join("");
  const cls = ["qbadge", `qbadge--${hint ?? "none"}`, size ? `qbadge--${size}` : ""]
    .filter(Boolean)
    .join(" ");
  return `<span class="${cls}"${title ? ` title="${esc(title)}"` : ""}><span class="qmeter" aria-hidden="true">${lit}</span>${esc(
    qualityLabel(bitrate, hint),
  )}</span>`;
}

export function coverCell(t: TrackSummary, cls = "cover"): string {
  if (t.coverUrl) {
    return `<img class="${cls}" src="${esc(t.coverUrl)}" alt="" loading="lazy">`;
  }
  return `<span class="${cls} cover--empty" role="img" aria-label="Không có ảnh bìa">${icon("music")}</span>`;
}

function skeletonRows(): string {
  const widths = [
    [70, 45],
    [55, 30],
    [80, 55],
    [62, 37],
    [74, 42],
  ];
  return widths
    .map(
      ([a, b]) =>
        `<div class="skeleton-row"><span class="skeleton skeleton--cover"></span><span class="skeleton-lines"><span class="skeleton" style="width:${a}%"></span><span class="skeleton" style="width:${b}%"></span></span><span class="skeleton skeleton--badge"></span></div>`,
    )
    .join("");
}

function row(t: TrackSummary): string {
  const bw = t.effectiveBandwidthHz
    ? `Băng thông thực ${(t.effectiveBandwidthHz / 1000).toFixed(2)} kHz`
    : "";
  return `<tr data-id="${esc(t.id)}" tabindex="-1" aria-selected="false">
    <td class="col-check"><input type="checkbox" aria-label="Chọn ${esc(t.title)}"></td>
    <td class="col-cover">${coverCell(t)}</td>
    <td class="col-title"><span class="cell-main">${esc(t.title)}</span><span class="cell-sub">${esc(t.artist ?? "Không rõ nghệ sĩ")}</span></td>
    <td class="col-album">${esc(t.album ?? "—")}</td>
    <td class="col-num num">${formatDuration(t.durationMs)}</td>
    <td class="col-quality">${qualityBadge(t.bitrate, t.qualityHint, "", bw)}</td>
    <td class="col-num num">${lufsNumber(t.integratedLufs)}</td>
    <td class="col-num col-last num">${formatSize(t.sizeBytes)}</td>
  </tr>`;
}

const TABLE_HEAD = `<colgroup><col class="w-check"><col class="w-cover"><col><col class="w-album"><col class="w-dur"><col class="w-quality"><col class="w-lufs"><col class="w-size"></colgroup>
  <thead><tr>
    <th class="col-check"><input type="checkbox" aria-label="Chọn tất cả" data-all></th>
    <th><span class="sr-only">Ảnh bìa</span></th>
    <th>Tiêu đề / Nghệ sĩ</th>
    <th>Album</th>
    <th class="col-num">Thời lượng</th>
    <th>Chất lượng</th>
    <th class="col-num" title="Integrated loudness">LUFS</th>
    <th class="col-num col-last">Dung lượng</th>
  </tr></thead>`;

export function mountLibrary(root: HTMLElement, cb: LibraryCallbacks): LibraryHandle {
  root.innerHTML = `<div data-slot="toolbar"></div><div data-slot="banner"></div><div class="table-wrap" data-slot="body"></div>`;
  const toolbarSlot = qs<HTMLElement>(root, '[data-slot="toolbar"]');
  const bannerSlot = qs<HTMLElement>(root, '[data-slot="banner"]');
  const body = qs<HTMLElement>(root, '[data-slot="body"]');

  let tracks: TrackSummary[] = [];
  let state: LibraryState = { kind: "ok", tracks: [] };
  let activeId: string | null = null;
  let focusId: string | null = null;
  const selected = new Set<string>();
  let loading = false;
  let seq = 0;

  const rowEl = (id: string) =>
    body.querySelector<HTMLTableRowElement>(`tr[data-id="${CSS.escape(id)}"]`);
  const rows = () => Array.from(body.querySelectorAll<HTMLTableRowElement>("tbody tr"));
  const writable = () => state.kind === "ok";

  function renderToolbar(): void {
    if (selected.size > 0) {
      const n = selected.size;
      toolbarSlot.innerHTML = `<div class="toolbar toolbar--selection" role="region" aria-label="Thao tác hàng loạt">
        <span class="selection-count">${icon("check")}Đã chọn <span class="num">${n}</span> bài</span>
        <button class="btn btn--ghost btn--sm" type="button" data-act="clear">Bỏ chọn</button>
        <div class="topbar-spacer"></div>
        <button class="btn btn--danger btn--sm" type="button" data-act="delete"${writable() ? "" : " disabled"}>${icon("trash")}Xoá ${n} bài</button>
      </div>`;
      return;
    }
    const total = tracks.reduce((sum, t) => sum + t.sizeBytes, 0);
    const meta = state.kind === "ok" && !loading ? `${tracks.length} bài · ${formatSize(total)}` : "";
    toolbarSlot.innerHTML = `<div class="toolbar">
      <h1 class="page-title">Thư viện</h1>
      <span class="toolbar-meta num">${meta}</span>
      <div class="topbar-spacer"></div>
      <button class="btn btn--ghost btn--sm" type="button" data-act="refresh"${loading ? " disabled" : ""}>${icon("refresh")}Làm mới</button>
    </div>`;
  }

  function renderBanner(): void {
    if (state.kind === "corrupt") {
      bannerSlot.innerHTML = `<div class="banner banner--danger" role="alert">${icon("alert-triangle")}<div><p class="banner-title">catalog.json bị lỗi</p><p class="banner-msg">Không đọc được catalog trên R2. Ứng dụng sẽ không ghi gì lên bucket cho tới khi catalog được sửa.</p><button class="btn btn--secondary btn--sm" type="button" data-act="refresh">${icon("refresh")}Thử lại</button></div></div>`;
    } else if (state.kind === "not-configured") {
      bannerSlot.innerHTML = `<div class="banner banner--info" role="status">${icon("info")}<div><p class="banner-title">Chưa cấu hình R2</p><p class="banner-msg">Nhập thông tin bucket trong Cài đặt để bắt đầu.</p></div></div>`;
    } else if (state.kind === "error") {
      bannerSlot.innerHTML = `<div class="banner banner--danger" role="alert">${icon("alert-triangle")}<div><p class="banner-title">Không tải được thư viện</p><p class="banner-msg">${esc(state.message)}</p><button class="btn btn--secondary btn--sm" type="button" data-act="refresh">${icon("refresh")}Thử lại</button></div></div>`;
    } else {
      bannerSlot.innerHTML = "";
    }
  }

  function renderBody(): void {
    body.setAttribute("aria-busy", String(loading));
    body.hidden = false;
    if (loading) {
      body.innerHTML = `<div class="skeleton-list" aria-label="Đang tải thư viện">${skeletonRows()}</div>`;
      return;
    }
    if (state.kind !== "ok") {
      body.innerHTML = "";
      body.hidden = true;
      return;
    }
    if (tracks.length === 0) {
      body.innerHTML = `<div class="empty"><span class="empty-icon">${icon("music", "icon--lg")}</span><p class="empty-title">Thư viện đang trống</p><p class="empty-desc">Kéo thả file MP3 vào cửa sổ hoặc bấm Tải lên.</p><button class="btn btn--primary btn--sm" type="button" data-act="upload">${icon("upload")}Tải lên</button></div>`;
      return;
    }
    body.innerHTML = `<table class="tracks">${TABLE_HEAD}<tbody>${tracks.map(row).join("")}</tbody></table>`;
    syncRows();
  }

  /** Applies selection / active / roving-tabindex state to the existing rows. */
  function syncRows(): void {
    const all = rows();
    const firstId = all[0]?.dataset.id ?? null;
    const tabId = focusId && rowEl(focusId) ? focusId : (activeId ?? firstId);
    for (const tr of all) {
      const id = tr.dataset.id ?? "";
      const isSel = selected.has(id);
      tr.classList.toggle("is-selected", isSel);
      tr.classList.toggle("is-active", id === activeId);
      tr.setAttribute("aria-selected", String(isSel));
      tr.tabIndex = id === tabId ? 0 : -1;
      qs<HTMLInputElement>(tr, 'input[type="checkbox"]').checked = isSel;
    }
    const head = body.querySelector<HTMLInputElement>("input[data-all]");
    if (head) {
      head.checked = all.length > 0 && selected.size === all.length;
      head.indeterminate = selected.size > 0 && selected.size < all.length;
    }
  }

  function selectionChanged(): void {
    syncRows();
    renderToolbar();
  }

  async function reload(): Promise<void> {
    const mine = ++seq;
    loading = true;
    renderToolbar();
    renderBanner();
    renderBody();
    let next: LibraryState;
    try {
      const list = await listTracks();
      next = { kind: "ok", tracks: list };
    } catch (e) {
      if (isNotConfigured(e)) next = { kind: "not-configured" };
      else if (isLibraryError(e) && e.code === "CATALOG_CORRUPT")
        next = { kind: "corrupt", message: e.message };
      else
        next = {
          kind: "error",
          message: isLibraryError(e) ? e.message : String(e),
        };
    }
    if (mine !== seq) return;
    loading = false;
    state = next;
    tracks = next.kind === "ok" ? next.tracks : [];
    for (const id of [...selected]) if (!tracks.some((t) => t.id === id)) selected.delete(id);
    renderToolbar();
    renderBanner();
    renderBody();
    cb.onState(state);
  }

  function activate(id: string): void {
    activeId = id;
    focusId = id;
    syncRows();
    cb.onOpen(id);
  }

  function toggle(id: string, on?: boolean): void {
    const next = on ?? !selected.has(id);
    if (next) selected.add(id);
    else selected.delete(id);
    selectionChanged();
  }

  root.addEventListener("click", (ev) => {
    const target = ev.target as HTMLElement;
    const act = target.closest<HTMLElement>("[data-act]")?.dataset.act;
    if (act === "refresh") void reload();
    else if (act === "clear") {
      selected.clear();
      selectionChanged();
    } else if (act === "delete") cb.onDeleteRequest([...selected]);
    else if (act === "upload") cb.onUploadRequest();
    else if (target.matches("input[data-all]")) {
      const on = (target as HTMLInputElement).checked;
      selected.clear();
      if (on) for (const t of tracks) selected.add(t.id);
      selectionChanged();
    } else if (target.matches('tbody input[type="checkbox"]')) {
      const id = target.closest<HTMLElement>("tr")?.dataset.id;
      if (id) toggle(id, (target as HTMLInputElement).checked);
    } else {
      const id = target.closest<HTMLElement>("tbody tr")?.dataset.id;
      if (id) activate(id);
    }
  });

  root.addEventListener("keydown", (ev) => {
    const tr = (ev.target as HTMLElement).closest<HTMLTableRowElement>("tbody tr");
    if (ev.key === "a" && (ev.ctrlKey || ev.metaKey) && body.querySelector("tbody")) {
      ev.preventDefault();
      for (const t of tracks) selected.add(t.id);
      selectionChanged();
      return;
    }
    if (!tr || (ev.target as HTMLElement).tagName === "INPUT") return;
    const id = tr.dataset.id ?? "";
    const all = rows();
    const idx = all.indexOf(tr);
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      const next = all[Math.max(0, Math.min(all.length - 1, idx + (ev.key === "ArrowDown" ? 1 : -1)))];
      focusId = next.dataset.id ?? null;
      syncRows();
      next.focus();
    } else if (ev.key === " ") {
      ev.preventDefault();
      toggle(id);
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      activate(id);
    } else if (ev.key === "Delete" && writable()) {
      cb.onDeleteRequest(selected.size > 0 ? [...selected] : [id]);
    }
  });

  return {
    reload,
    setActive(id) {
      activeId = id;
      syncRows();
    },
    selectedIds: () => [...selected],
    clearSelection() {
      if (selected.size === 0) return false;
      selected.clear();
      selectionChanged();
      return true;
    },
    tracks: () => tracks,
  };
}
