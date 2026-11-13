// The progress band: one adinkra symbol is stamped per step, like a stamped adinkra cloth.
// Dwennimmen (ram's horns: strength with humility) and Nkyinkyim (the twisting path), drawn as
// fixed SVG strings (not user or chain data). Replaces the numbered step tracker; the step
// number and name are still given in text, so nothing depends on the picture.
import { h } from "./dom.js";
import { reducedMotion } from "./motion.js";

const HORN = "M21.5 21.5L17.6 17.6L18.2 16.8L18.6 15.9L19 14.9L19.2 14L19.2 13L19.1 12.1L18.9 11.2L18.6 10.3L18.1 9.5L17.6 8.8L16.9 8.2L16.2 7.7L15.5 7.3L14.7 7L13.8 6.9L13 6.8L12.2 6.9L11.5 7.1L10.7 7.4L10.1 7.8L9.5 8.2L9 8.8L8.6 9.3L8.2 10L8 10.6L7.9 11.3L7.8 12L7.9 12.6L8.1 13.3L8.3 13.9L8.6 14.4L9 14.9L9.4 15.3L9.9 15.6L10.4 15.8L11 16L11.5 16.1L12 16.1L12.6 16.1L13 15.9L13.5 15.7L13.9 15.5L14.2 15.2L14.6 14.8L14.8 14.4L14.9 14.1L15.1 13.7L15.1 13.3L15.1 12.9L15 12.5L14.9 12.2L14.8 11.8L14.6 11.6L14.3 11.3L14.1 11.2L13.8 11L13.5 10.9L13.3 10.9L13 10.9L12.8 10.9";
const DWENNIMMEN = [0, 90, 180, 270].map((r) => `<path d="${HORN}" transform="rotate(${r} 24 24)"/>`).join("")
  + '<circle cx="24" cy="24" r="3.4" class="fill"/>';
const NKYINKYIM = '<path d="M12 5h10l14 9.5-14 9.5 14 9.5-14 9.5H12l14-9.5-14-9.5 14-9.5z"/>';
const SYMBOLS = [DWENNIMMEN, NKYINKYIM, DWENNIMMEN, NKYINKYIM];
const STEPS = ["Choose amount", "Pay by MoMo", "Paystack check", "USDC received"];

let lastStamped = 0; // the highest step already shown stamped, so a re-render doesn't stamp again

function symbol(markup, cls) {
  const t = document.createElement("template");
  t.innerHTML = `<svg class="${cls}" viewBox="0 0 48 48" aria-hidden="true" focusable="false">${markup}</svg>`;
  return t.content.firstChild;
}

/** `step` 1-4. `ended`: the order stopped (expired); nothing after `step` gets stamped. */
export function band(step, ended = false) {
  const stamp = !ended && step > lastStamped && !reducedMotion();
  if (!ended) lastStamped = Math.max(lastStamped, step);
  const marks = SYMBOLS.map((markup, i) => {
    const n = i + 1;
    const done = ended ? n < step : n <= step;
    return symbol(markup, `mark${done ? " on" : ""}${stamp && n === step ? " stamp" : ""}`);
  });
  const label = ended ? `Stopped at step ${step} of 4` : `Step ${step} of 4: ${STEPS[step - 1]}`;
  return h("div", { class: "band", role: "img", "aria-label": label },
    h("div", { class: "marks" }, marks),
    h("p", { class: "band-label" }, ended ? "Order closed" : `Step ${step} of 4 · ${STEPS[step - 1]}`),
  );
}
