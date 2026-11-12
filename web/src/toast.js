// Short messages. Errors are announced immediately; nothing here shows raw JSON.
import { h } from "./dom.js";

let host;

/** kind: "ok" | "error" | "pending". link: optional { href, text }. Returns a dismiss function. */
export function toast(kind, message, link) {
  if (!host) {
    host = h("div", { class: "toasts" });
    document.body.append(host);
  }
  const el = h("div", { class: `toast ${kind}`, role: kind === "error" ? "alert" : "status" },
    h("span", { class: "bar" }),
    h("div", { class: "body" },
      message,
      link ? h("a", { href: link.href, target: "_blank", rel: "noopener" }, link.text) : null,
    ),
    h("button", { class: "x", "aria-label": "Dismiss", onclick: () => dismiss() }, "×"),
  );
  let gone = false;
  const dismiss = () => {
    if (gone) return;
    gone = true;
    el.classList.add("leaving");
    setTimeout(() => el.remove(), 150);
  };
  host.append(el);
  while (host.children.length > 3) host.firstChild.remove();
  if (kind !== "pending") setTimeout(dismiss, kind === "error" ? 9000 : 6000);
  return dismiss;
}
