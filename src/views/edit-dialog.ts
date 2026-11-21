import { pickImage, updateTrack } from "../api";
import { esc, icon, qs, trapFocus } from "../dom";
import type { TagEdit, TrackSummary } from "../types";
import { isLibraryError } from "../types";
import { coverCell } from "./library";

interface Field {
  key: keyof TagEdit;
  id: string;
  label: string;
  span: number;
  cls?: string;
  inputmode?: string;
}

const FIELDS: Field[] = [
  { key: "title", id: "e-title", label: "Tiêu đề", span: 6 },
  { key: "artist", id: "e-artist", label: "Nghệ sĩ", span: 3 },
  { key: "albumArtist", id: "e-aa", label: "Nghệ sĩ album", span: 3 },
  { key: "album", id: "e-album", label: "Album", span: 4 },
  { key: "date", id: "e-date", label: "Ngày phát hành", span: 2 },
  { key: "genre", id: "e-genre", label: "Thể loại", span: 4 },
  { key: "track", id: "e-track", label: "Track", span: 1, cls: " num", inputmode: "numeric" },
  { key: "disc", id: "e-disc", label: "Disc", span: 1, cls: " num", inputmode: "numeric" },
];

function fieldHtml(f: Field, t: TrackSummary): string {
  const req = f.key === "title";
  return `<div class="field" style="grid-column:span ${f.span}">
    <label class="field-label" for="${f.id}">${f.label}${req ? ' <span class="req" aria-hidden="true">*</span>' : ""}</label>
    <input class="input${f.cls ?? ""}" id="${f.id}" name="${f.key}" value="${esc(t[f.key])}"${req ? " required" : ""}${f.inputmode ? ` inputmode="${f.inputmode}"` : ""} autocomplete="off">
  </div>`;
}

/**
 * Edit-tags modal. Resolves with the updated track after a successful save,
 * or null when cancelled.
 */
export function openEditDialog(track: TrackSummary): Promise<TrackSummary | null> {
  const slot = qs<HTMLElement>(document, "#overlay-slot");
  const scrim = document.createElement("div");
  scrim.className = "scrim";
  scrim.innerHTML = `<form class="dialog" role="dialog" aria-modal="true" aria-labelledby="edit-title" novalidate>
    <div class="dialog-head">
      <div>
        <h2 class="dialog-title" id="edit-title">Sửa thông tin bài hát</h2>
        <p class="dialog-desc">Chỉ đổi tag và ảnh bìa. File nhạc trên R2 giữ nguyên.</p>
      </div>
      <button class="btn btn--ghost btn--icon" type="button" data-act="cancel" aria-label="Đóng">${icon("x")}</button>
    </div>
    <div class="dialog-body">
      <div class="cover-edit">
        ${coverCell(track, "cover cover--edit")}
        <div>
          <p class="field-label">Ảnh bìa</p>
          <p class="field-help" data-slot="cover-help">JPG hoặc PNG. Ảnh mới được lưu dưới khoá mới, ảnh cũ bị xoá nếu không còn bài nào dùng.</p>
          <button class="btn btn--secondary btn--sm" type="button" data-act="cover">${icon("image")}Đổi ảnh bìa…</button>
        </div>
      </div>
      <div class="form-grid">${FIELDS.map((f) => fieldHtml(f, track)).join("")}</div>
      <p class="field-error" id="e-error" role="alert" data-slot="error" style="margin-top:var(--space-3)" hidden></p>
    </div>
    <div class="dialog-foot">
      <span class="dialog-foot-note"><span class="req">*</span> bắt buộc</span>
      <button class="btn btn--secondary" type="button" data-act="cancel">Huỷ</button>
      <button class="btn btn--primary" type="submit">Lưu thay đổi</button>
    </div>
  </form>`;

  return new Promise((resolve) => {
    const opener = document.activeElement as HTMLElement | null;
    const form = qs<HTMLFormElement>(scrim, "form");
    const titleInput = qs<HTMLInputElement>(scrim, "#e-title");
    const errorEl = qs<HTMLElement>(scrim, '[data-slot="error"]');
    const help = qs<HTMLElement>(scrim, '[data-slot="cover-help"]');
    const saveBtn = qs<HTMLButtonElement>(scrim, 'button[type="submit"]');
    let coverPath: string | null = null;
    let saving = false;
    let release = () => {};

    const finish = (value: TrackSummary | null) => {
      release();
      scrim.remove();
      resolve(value);
    };
    const showError = (msg: string, onTitle = false) => {
      errorEl.innerHTML = `${icon("alert-circle")}${esc(msg)}`;
      errorEl.hidden = false;
      if (onTitle) {
        titleInput.setAttribute("aria-invalid", "true");
        titleInput.setAttribute("aria-describedby", "e-error");
        titleInput.focus();
      }
    };

    titleInput.addEventListener("input", () => {
      titleInput.removeAttribute("aria-invalid");
      errorEl.hidden = true;
    });

    scrim.addEventListener("click", async (ev) => {
      const act = (ev.target as HTMLElement).closest<HTMLElement>("[data-act]")?.dataset.act;
      if (act === "cancel" && !saving) finish(null);
      else if (act === "cover") {
        const picked = await pickImage();
        if (picked) {
          coverPath = picked;
          help.textContent = `Ảnh bìa mới: ${picked.split(/[\\/]/).pop() ?? picked}`;
        }
      }
    });

    form.addEventListener("submit", async (ev) => {
      ev.preventDefault();
      if (saving) return;
      const value = (key: keyof TagEdit) =>
        qs<HTMLInputElement>(scrim, `input[name="${key}"]`).value.trim();
      const optional = (key: keyof TagEdit) => value(key) || null;
      const title = value("title");
      if (!title) {
        showError("Tiêu đề không được để trống.", true);
        return;
      }
      const edit: TagEdit = {
        title,
        artist: optional("artist"),
        album: optional("album"),
        albumArtist: optional("albumArtist"),
        date: optional("date"),
        genre: optional("genre"),
        track: optional("track"),
        disc: optional("disc"),
      };
      saving = true;
      saveBtn.disabled = true;
      saveBtn.classList.add("is-loading");
      try {
        finish(await updateTrack(track.id, edit, coverPath));
      } catch (e) {
        saving = false;
        saveBtn.disabled = false;
        saveBtn.classList.remove("is-loading");
        showError(isLibraryError(e) ? e.message : String(e));
      }
    });

    slot.append(scrim);
    release = trapFocus(scrim, () => !saving && finish(null), opener);
    titleInput.focus();
    titleInput.select();
  });
}
