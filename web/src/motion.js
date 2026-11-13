// Plain CSS + Web Animations. Skipped when the user asks for reduced motion.
// Nothing here blocks a click, and nothing runs longer than 400 ms.

export const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Count `from` → `to` (BigInt) over 300 ms, writing fmt(value) into el. */
export function countTo(el, from, to, fmt) {
  if (el._raf) cancelAnimationFrame(el._raf);
  el.textContent = fmt(to);
  el._n = to;
  if (reducedMotion() || from === to) return;
  const start = performance.now();
  const diff = Number(to - from);
  const tick = (t) => {
    const p = Math.min(1, (t - start) / 300);
    const eased = 1 - (1 - p) ** 3;
    const value = p === 1 ? to : from + BigInt(Math.round(diff * eased));
    el.textContent = fmt(value);
    el._n = value;
    if (p < 1) el._raf = requestAnimationFrame(tick);
  };
  el._raf = requestAnimationFrame(tick);
}

/** One small burst inside `host`. Removes itself. */
export function confetti(host) {
  if (reducedMotion() || !host?.animate) return;
  const box = document.createElement("div");
  box.className = "confetti";
  box.setAttribute("aria-hidden", "true");
  const colours = ["var(--brand)", "var(--ink)", "var(--ink-2)"];
  for (let i = 0; i < 18; i++) {
    const piece = document.createElement("i");
    piece.style.background = colours[i % colours.length];
    box.append(piece);
    const angle = (Math.PI * 2 * i) / 18;
    const dist = 56 + (i % 5) * 14;
    piece.animate(
      [
        { transform: "translate(0,0) rotate(0deg)", opacity: 1 },
        { transform: `translate(${Math.cos(angle) * dist}px, ${Math.sin(angle) * dist - 24}px) rotate(180deg)`, opacity: 0 },
      ],
      { duration: 400, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)", fill: "forwards" },
    );
  }
  host.append(box);
  setTimeout(() => box.remove(), 420);
}
