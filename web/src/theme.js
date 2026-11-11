// Light / dark: follows the system until the user picks one. The choice stays in this browser only.
const KEY = "openramp.theme";

const systemDark = () => window.matchMedia("(prefers-color-scheme: dark)").matches;

export function currentTheme() {
  return document.documentElement.dataset.theme || (systemDark() ? "dark" : "light");
}

export function initTheme() {
  const q = new URLSearchParams(location.search).get("theme");
  if (q === "light" || q === "dark") {
    document.documentElement.dataset.theme = q;
    try { localStorage.setItem(KEY, q); } catch { /* choice lasts for this visit */ }
    return;
  }
  let saved = null;
  try { saved = localStorage.getItem(KEY); } catch { /* storage blocked: follow the system */ }
  if (saved === "light" || saved === "dark") document.documentElement.dataset.theme = saved;
}

export function toggleTheme() {
  const next = currentTheme() === "dark" ? "light" : "dark";
  document.documentElement.dataset.theme = next;
  try { localStorage.setItem(KEY, next); } catch { /* choice lasts for this visit */ }
  return next;
}
