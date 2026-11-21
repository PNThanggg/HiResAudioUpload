import { getSettings, saveSettings, testConnection } from "../api";
import { esc, icon, qs, toast } from "../dom";
import type { Settings } from "../types";

export interface SettingsCallbacks {
  /** Called after a successful save. */
  onSaved(): void;
}

const MASK_PREFIX = "••••";

interface FieldDef {
  key: keyof Settings;
  id: string;
  label: string;
  help?: string;
  type?: string;
}

const FIELDS: FieldDef[] = [
  { key: "accountId", id: "s-acc", label: "Account ID" },
  { key: "accessKeyId", id: "s-akid", label: "Access Key ID" },
  { key: "secretAccessKey", id: "s-secret", label: "Secret Access Key" },
  { key: "bucket", id: "s-bucket", label: "Bucket" },
  {
    key: "publicBaseUrl",
    id: "s-url",
    label: "Public base URL",
    type: "url",
    help: 'App Android đọc <span class="mono">catalog.json</span> và file nhạc qua URL này.',
  },
];

export function isSettingsComplete(s: Settings): boolean {
  return Object.values(s).every((v) => v.trim().length > 0);
}

function field(f: FieldDef, value: string): string {
  const err = `<p class="field-error" id="${f.id}-err" hidden>${icon("alert-circle")}Trường này là bắt buộc.</p>`;
  const common = `class="input mono" id="${f.id}" name="${f.key}" spellcheck="false" autocomplete="off" required aria-describedby="${f.id}-err"`;
  if (f.key === "secretAccessKey") {
    return `<div class="field">
      <label class="field-label" for="${f.id}">${f.label} <span class="req" aria-hidden="true">*</span></label>
      <div class="input-wrap"><input ${common} type="text" value="${esc(value)}"><button class="btn btn--ghost btn--icon btn--xs input-action" type="button" data-act="reveal" aria-label="Hiện secret" aria-pressed="false">${icon("eye")}</button></div>
      <p class="field-help" data-secret-help></p>
      ${err}
    </div>`;
  }
  return `<div class="field">
    <label class="field-label" for="${f.id}">${f.label} <span class="req" aria-hidden="true">*</span></label>
    <input ${common} type="${f.type ?? "text"}" value="${esc(value)}">
    ${f.help ? `<p class="field-help">${f.help}</p>` : ""}
    ${err}
  </div>`;
}

export async function mountSettings(
  root: HTMLElement,
  opts: { configured: boolean } & SettingsCallbacks,
): Promise<void> {
  const current = await getSettings();
  const stored = current.secretAccessKey;
  const banner = opts.configured
    ? ""
    : `<div class="banner banner--info" role="status">${icon("info")}<div><p class="banner-title">Chưa cấu hình R2</p><p class="banner-msg">Nhập thông tin bucket để bắt đầu.</p></div></div>`;
  root.innerHTML = `<main class="page">
    <div class="page-head">
      <h1 class="page-title page-title--lg">Cài đặt</h1>
      <p class="page-desc">Kết nối tới bucket Cloudflare R2 chứa thư viện nhạc.</p>
    </div>
    ${banner}
    <div class="settings-layout">
      <form class="card settings-form" novalidate>
        <h2 class="card-title">${icon("cloud")}Cloudflare R2</h2>
        ${FIELDS.map((f) => field(f, current[f.key])).join("")}
        <div class="form-actions">
          <p class="test-result" role="status" data-slot="result"></p>
          <button class="btn btn--secondary" type="button" data-act="test">${icon("refresh")}Kiểm tra kết nối</button>
          <button class="btn btn--primary" type="submit" data-act="save">Lưu</button>
        </div>
      </form>
      <aside class="card card--note">
        <h2 class="card-title">${icon("info")}Trước khi kết nối</h2>
        <ul class="note-list">
          <li>Bucket cần bật public access (r2.dev hoặc domain riêng).</li>
          <li>Không bật nén cho audio và ảnh. Range request mặc định đã hoạt động.</li>
          <li>Key chỉ nằm trong máy này, app Android không bao giờ cầm key.</li>
        </ul>
      </aside>
    </div>
  </main>`;

  const form = qs<HTMLFormElement>(root, "form");
  const result = qs<HTMLElement>(root, '[data-slot="result"]');
  const secret = qs<HTMLInputElement>(root, "#s-secret");
  const reveal = qs<HTMLButtonElement>(root, '[data-act="reveal"]');
  const testBtn = qs<HTMLButtonElement>(root, '[data-act="test"]');
  const saveBtn = qs<HTMLButtonElement>(root, '[data-act="save"]');
  const help = qs<HTMLElement>(root, "[data-secret-help]");
  let revealed = false;

  const input = (f: FieldDef) => qs<HTMLInputElement>(root, `#${f.id}`);
  const read = (): Settings => ({
    accountId: input(FIELDS[0]).value,
    accessKeyId: input(FIELDS[1]).value,
    secretAccessKey: secret.value,
    bucket: input(FIELDS[3]).value,
    publicBaseUrl: input(FIELDS[4]).value,
  });

  /** The masked secret is shown as text; a newly typed one is hidden unless revealed. */
  function syncSecret(): void {
    const masked = stored.startsWith(MASK_PREFIX) && secret.value === stored;
    secret.type = masked || revealed ? "text" : "password";
    help.innerHTML = masked
      ? `Đang lưu secret kết thúc bằng <span class="mono">${esc(stored.slice(MASK_PREFIX.length))}</span>. Để nguyên để giữ, hoặc nhập giá trị mới.`
      : "";
    help.hidden = !masked;
  }
  syncSecret();

  secret.addEventListener("input", syncSecret);
  reveal.addEventListener("click", () => {
    revealed = !revealed;
    reveal.setAttribute("aria-pressed", String(revealed));
    syncSecret();
  });

  function setResult(kind: "ok" | "error" | "", html: string): void {
    result.className = `test-result${kind ? ` test-result--${kind}` : ""}`;
    result.innerHTML = html;
  }

  function validate(): boolean {
    let ok = true;
    for (const f of FIELDS) {
      const el = input(f);
      const bad = el.value.trim() === "";
      el.setAttribute("aria-invalid", String(bad));
      qs<HTMLElement>(root, `#${f.id}-err`).hidden = !bad;
      ok &&= !bad;
    }
    return ok;
  }

  function setBusy(btn: HTMLButtonElement, busy: boolean, label: string, idle: string): void {
    testBtn.disabled = busy;
    saveBtn.disabled = busy;
    btn.classList.toggle("is-loading", busy);
    btn.setAttribute("aria-busy", String(busy));
    btn.innerHTML = busy ? `${icon("loader", "spin")}${label}` : idle;
  }

  testBtn.addEventListener("click", async () => {
    if (!validate()) return;
    const idle = `${icon("refresh")}Kiểm tra kết nối`;
    setResult("", "");
    setBusy(testBtn, true, "Đang kiểm tra…", idle);
    try {
      await testConnection(read());
      setResult("ok", `${icon("check-circle")}<span>Kết nối thành công · đọc được <span class="mono">catalog.json</span></span>`);
    } catch (e) {
      setResult("error", `${icon("alert-circle")}<span>${esc(typeof e === "string" ? e : String((e as { message?: string }).message ?? e))}</span>`);
    } finally {
      setBusy(testBtn, false, "", idle);
    }
  });

  form.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (!validate()) return;
    setResult("", "");
    setBusy(saveBtn, true, "Đang lưu…", "Lưu");
    try {
      await saveSettings(read());
      toast("success", "Đã lưu cài đặt", "Thư viện được tải lại từ bucket.");
      opts.onSaved();
    } catch (e) {
      setResult("error", `${icon("alert-circle")}<span>${esc(typeof e === "string" ? e : String(e))}</span>`);
    } finally {
      setBusy(saveBtn, false, "", "Lưu");
    }
  });
}
