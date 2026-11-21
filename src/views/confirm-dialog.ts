import { esc, icon, qs, trapFocus } from "../dom";
import type { TrackSummary } from "../types";

const MAX_LISTED = 5;

/** Danger confirm for deleting tracks. Resolves true on confirm, false on cancel/Esc. */
export function confirmDelete(tracks: TrackSummary[]): Promise<boolean> {
  const slot = qs<HTMLElement>(document, "#overlay-slot");
  const n = tracks.length;
  const listed = tracks.slice(0, MAX_LISTED);
  const rest = n - listed.length;
  const items = listed
    .map((t) => `<li><b>${esc(t.title)}</b>${t.artist ? ` · ${esc(t.artist)}` : ""}</li>`)
    .join("");

  const scrim = document.createElement("div");
  scrim.className = "scrim";
  scrim.innerHTML = `<div class="dialog dialog--sm" role="alertdialog" aria-modal="true" aria-labelledby="del-title" aria-describedby="del-desc">
    <div class="dialog-head"><div>
      <h2 class="dialog-title" id="del-title">Xoá ${n} bài hát?</h2>
      <p class="dialog-desc" id="del-desc">File nhạc, ảnh bìa và dữ liệu trên R2 sẽ bị xoá. App Android sẽ không thấy ${n === 1 ? "bài này" : "các bài này"} nữa. Không thể hoàn tác.</p>
    </div></div>
    <div class="dialog-body"><ul class="del-list">${items}${rest > 0 ? `<li>và ${rest} bài khác</li>` : ""}</ul></div>
    <div class="dialog-foot">
      <button class="btn btn--secondary" type="button" data-act="cancel">Huỷ</button>
      <button class="btn btn--danger" type="button" data-act="confirm">${icon("trash")}Xoá ${n} bài</button>
    </div>
  </div>`;

  return new Promise((resolve) => {
    const opener = document.activeElement as HTMLElement | null;
    let release = () => {};
    const finish = (value: boolean) => {
      release();
      scrim.remove();
      resolve(value);
    };
    scrim.addEventListener("click", (ev) => {
      const target = ev.target as HTMLElement;
      const act = target.closest<HTMLElement>("[data-act]")?.dataset.act;
      if (act === "confirm") finish(true);
      else if (act === "cancel" || target === scrim) finish(false);
    });
    slot.append(scrim);
    release = trapFocus(scrim, () => finish(false), opener);
    qs<HTMLButtonElement>(scrim, '[data-act="cancel"]').focus();
  });
}
