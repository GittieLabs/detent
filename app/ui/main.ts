// Stub settings window for M0. The Devices, Mappings, and Test screens land in M3 and M4.

const root = document.querySelector<HTMLElement>("#app");
if (root) {
  root.innerHTML = `
    <h1>Detent</h1>
    <p>Settings arrive in a later milestone. Detent runs from the menu bar.</p>
  `;
  root.style.fontFamily = "-apple-system, system-ui, sans-serif";
  root.style.padding = "24px";
}
