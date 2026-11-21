import "./styles/tokens.css";
import "./styles/app.css";
import { getSettings } from "./api";
import { esc, icon, qs } from "./dom";
import { mountDetail, type DetailHandle } from "./views/detail";
import { mountLibrary, type LibraryHandle, type LibraryState } from "./views/library";
import { isSettingsComplete, mountSettings } from "./views/settings";

type Route = "library" | "settings";
/** Kinds of content the right-hand inspector panel can show. Task 13 adds "upload". */
export type PanelKind = "detail" | "upload";

const inTauri = "__TAURI_INTERNALS__" in window;
// Dev/test hooks, only honoured in the browser mock: ?theme=light|dark, ?route=settings, ?open=<row index>.
const query = new URLSearchParams(inTauri ? "" : window.location.search);
const forcedTheme = query.get("theme");
if (forcedTheme === "light" || forcedTheme === "dark") {
  document.documentElement.setAttribute("data-theme", forcedTheme);
}

const app = qs<HTMLElement>(document, "#app");
app.innerHTML = `<header class="topbar" data-slot="topbar"></header>
  <div class="main main--full" data-slot="library-view">
    <section class="library" aria-label="Thư viện"></section>
    <aside class="inspector" aria-label="Chi tiết bài hát" hidden></aside>
  </div>
  <div data-slot="settings-view" hidden></div>`;

const topbar = qs<HTMLElement>(app, '[data-slot="topbar"]');
const libraryView = qs<HTMLElement>(app, '[data-slot="library-view"]');
const settingsView = qs<HTMLElement>(app, '[data-slot="settings-view"]');
const librarySlot = qs<HTMLElement>(libraryView, ".library");
const panelSlot = qs<HTMLElement>(libraryView, ".inspector");

let route: Route = "library";
let panel: PanelKind | null = null;
let detail: DetailHandle | null = null;
let bucket = "";
let count: number | null = null;
let conn: LibraryState["kind"] | "connecting" = "connecting";
let library: LibraryHandle;

function connLabel(): { cls: string; html: string } {
  switch (conn) {
    case "ok":
      return { cls: "", html: `Đã kết nối · <span class="mono">${esc(bucket)}</span>` };
    case "not-configured":
      return { cls: " conn--off", html: "Chưa cấu hình" };
    case "corrupt":
      return { cls: " conn--error", html: "Catalog bị lỗi" };
    case "error":
      return { cls: " conn--error", html: "Không kết nối được" };
    default:
      return { cls: " conn--off", html: "Đang kết nối…" };
  }
}

function renderTopbar(): void {
  const c = connLabel();
  const cur = (r: Route) => (route === r ? ' aria-current="page"' : "");
  topbar.innerHTML = `<div class="brand"><span class="brand-mark">${icon("wave")}</span>HiResAudio Upload</div>
    <nav class="nav" aria-label="Điều hướng chính">
      <a class="nav-item" href="#" data-route="library"${cur("library")}>${icon("library")}Thư viện${count === null ? "" : ` <span class="count num">${count}</span>`}</a>
      <a class="nav-item" href="#" data-route="settings"${cur("settings")}>${icon("settings")}Cài đặt</a>
    </nav>
    <div class="topbar-spacer"></div>
    <span class="conn${c.cls}"><span class="conn-dot" aria-hidden="true"></span>${c.html}</span>
    <button class="btn btn--primary" type="button" data-act="upload" aria-expanded="${panel === "upload"}"${conn === "corrupt" ? " disabled" : ""}>${icon("upload")}Tải lên</button>`;
}

/**
 * Opens (or closes, with `null`) the right-hand inspector panel.
 * Task 13 hooks the upload panel in here: render into `panelSlot` for `"upload"`.
 */
export function showPanel(kind: PanelKind | null, arg?: string): void {
  if (kind === "upload") {
    // Task 13: mount the upload panel into panelSlot.
    return;
  }
  detail?.dispose();
  detail = null;
  panel = kind;
  panelSlot.hidden = kind === null;
  libraryView.classList.toggle("main--full", kind === null);
  if (kind === "detail" && arg) {
    library.setActive(arg);
    detail = mountDetail(panelSlot, arg, {
      onClose: () => showPanel(null),
      onEdit: () => {}, // Task 13: edit dialog
      onDelete: () => {}, // Task 13: delete confirm
    });
  } else {
    panelSlot.innerHTML = "";
    library.setActive(null);
  }
  renderTopbar();
}

async function refreshBucket(): Promise<void> {
  try {
    bucket = (await getSettings()).bucket;
  } catch {
    bucket = "";
  }
}

async function goTo(next: Route): Promise<void> {
  route = next;
  libraryView.hidden = next !== "library";
  settingsView.hidden = next !== "settings";
  renderTopbar();
  if (next === "settings") {
    const configured = isSettingsComplete(await getSettings());
    await mountSettings(settingsView, {
      configured,
      onSaved: () => {
        void refreshBucket().then(() => goTo("library"));
      },
    });
  } else {
    await refreshBucket();
    await library.reload();
  }
}

topbar.addEventListener("click", (ev) => {
  const target = ev.target as HTMLElement;
  const link = target.closest<HTMLElement>("[data-route]");
  if (link) {
    ev.preventDefault();
    const next = link.dataset.route as Route;
    if (next !== route) void goTo(next);
  } else if (target.closest('[data-act="upload"]')) {
    showPanel("upload"); // no-op until Task 13
  }
});

document.addEventListener("keydown", (ev) => {
  if (ev.key !== "Escape" || route !== "library") return;
  if (document.querySelector("dialog[open]")) return;
  if (!library.clearSelection() && panel !== null) showPanel(null);
});

library = mountLibrary(librarySlot, {
  onOpen: (id) => showPanel("detail", id),
  onDeleteRequest: () => {}, // Task 13: delete confirm dialog
  onUploadRequest: () => showPanel("upload"),
  onState: (state) => {
    conn = state.kind;
    count = state.kind === "ok" ? state.tracks.length : null;
    renderTopbar();
    if (state.kind === "ok") {
      const open = Number(query.get("open"));
      if (query.has("open") && state.tracks[open]) {
        query.delete("open");
        showPanel("detail", state.tracks[open].id);
      }
    }
  },
});

void (async () => {
  const settings = await getSettings();
  bucket = settings.bucket;
  if (!isSettingsComplete(settings)) conn = "not-configured";
  const start: Route =
    query.get("route") === "settings" || !isSettingsComplete(settings) ? "settings" : "library";
  renderTopbar();
  // Populate the count and connection status even when starting on Settings.
  if (start === "settings" && isSettingsComplete(settings)) void library.reload();
  await goTo(start);
})();
