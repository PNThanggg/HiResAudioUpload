const ESCAPES: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
};

export function esc(value: string | number | null | undefined): string {
  return String(value ?? "").replace(/[&<>"']/g, (c) => ESCAPES[c]);
}

/** Inline SVG icon from the sprite defined in index.html. */
export function icon(name: string, extra = ""): string {
  return `<svg class="icon${extra ? ` ${extra}` : ""}" aria-hidden="true"><use href="#i-${name}"/></svg>`;
}

export function qs<T extends Element>(root: ParentNode, selector: string): T {
  const found = root.querySelector<T>(selector);
  if (!found) throw new Error(`missing element: ${selector}`);
  return found;
}

export type ToastKind = "success" | "error" | "info";

const TOAST_ICON: Record<ToastKind, string> = {
  success: "check-circle",
  error: "alert-circle",
  info: "info",
};

/** Shows a toast in the bottom-right region (max 3, newest last). */
export function toast(kind: ToastKind, title: string, message = ""): void {
  const region = document.getElementById("toast-region");
  if (!region) return;
  const el = document.createElement("div");
  el.className = `toast${kind === "info" ? "" : ` toast--${kind}`}`;
  el.setAttribute("role", kind === "error" ? "alert" : "status");
  el.innerHTML = `${icon(TOAST_ICON[kind])}<div><p class="toast-title">${esc(title)}</p>${
    message ? `<p class="toast-msg">${esc(message)}</p>` : ""
  }</div><button class="btn btn--ghost btn--icon btn--xs" type="button" aria-label="Đóng thông báo">${icon("x")}</button>`;
  region.append(el);
  while (region.children.length > 3) region.firstElementChild?.remove();

  const remove = () => el.remove();
  let timer = window.setTimeout(remove, kind === "error" ? 8000 : 4000);
  el.addEventListener("mouseenter", () => window.clearTimeout(timer));
  el.addEventListener("mouseleave", () => {
    timer = window.setTimeout(remove, 4000);
  });
  qs<HTMLButtonElement>(el, "button").addEventListener("click", remove);
}
