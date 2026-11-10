// The volume overlay (D-083): a level bar shown briefly after a volume
// action. Its window has no IPC capabilities; Rust sets the first value with
// an initialization script and later ones with `window.eval`. Built with DOM
// calls, not markup strings, so the CSP needs no inline styles.

import "./hud.css";

declare global {
  interface Window {
    __hudInit?: [number, boolean | null];
    detentHud?: (level: number, muted: boolean | null) => void;
  }
}

const SVG = "http://www.w3.org/2000/svg";

function speaker(muted: boolean): SVGElement {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  const paths = muted
    ? ["M4 9h4l5-4v14l-5-4H4z", "M16 9l5 6", "M21 9l-5 6"]
    : ["M4 9h4l5-4v14l-5-4H4z", "M16 9a4 4 0 0 1 0 6", "M18.5 6.5a8 8 0 0 1 0 11"];
  for (const d of paths) {
    const p = document.createElementNS(SVG, "path");
    p.setAttribute("d", d);
    svg.append(p);
  }
  return svg;
}

function render(level: number, muted: boolean | null) {
  const hud = document.getElementById("hud");
  if (!hud) return;
  const off = muted === true;
  const pct = Math.max(0, Math.min(100, Math.round(level)));
  const bar = document.createElement("div");
  bar.className = "bar";
  const fill = document.createElement("div");
  fill.className = "fill";
  fill.style.width = `${off ? 0 : pct}%`;
  bar.append(fill);
  const label = document.createElement("span");
  label.className = "pct";
  label.textContent = off ? "Muted" : `${pct}%`;
  hud.replaceChildren(speaker(off), bar, label);
  hud.setAttribute("aria-label", off ? "Muted" : `Volume ${pct}%`);
}

window.detentHud = render;
const init = window.__hudInit;
if (init) render(init[0], init[1]);

export {};
