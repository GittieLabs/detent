// A tiny element builder. Text always goes in through `textContent`, never
// `innerHTML`, so a device name can't inject markup.

type Child = Node | string | null | undefined | false;

export interface Props {
  class?: string;
  title?: string;
  disabled?: boolean;
  type?: string;
  value?: string;
  checked?: boolean;
  ariaLabel?: string;
  onClick?: (event: MouseEvent) => void;
  onKey?: (event: KeyboardEvent) => void;
  onInput?: (value: string) => void;
  /** Other attributes, set as given (min, max, step, rows, placeholder...). */
  attrs?: Record<string, string>;
}

export function el(tag: string, props: Props = {}, ...children: Child[]): HTMLElement {
  const node = document.createElement(tag);
  if (props.class) node.className = props.class;
  if (props.title) node.title = props.title;
  if (props.ariaLabel) node.setAttribute("aria-label", props.ariaLabel);
  if (props.type) node.setAttribute("type", props.type);
  if (props.disabled) node.setAttribute("disabled", "");
  if (props.checked && node instanceof HTMLInputElement) node.checked = true;
  for (const [name, value] of Object.entries(props.attrs ?? {})) node.setAttribute(name, value);
  if (props.onClick) node.addEventListener("click", props.onClick);
  if (props.onKey) node.addEventListener("keydown", props.onKey);
  const field =
    node instanceof HTMLInputElement ||
    node instanceof HTMLSelectElement ||
    node instanceof HTMLTextAreaElement
      ? node
      : null;
  const onInput = props.onInput;
  if (onInput && field) field.addEventListener("input", () => onInput(field.value));
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    node.append(typeof child === "string" ? document.createTextNode(child) : child);
  }
  // After the children, so a select's options exist to be chosen.
  if (props.value !== undefined && field) field.value = props.value;
  if (props.value !== undefined && node instanceof HTMLOptionElement) node.value = props.value;
  return node;
}

export function button(label: string, onClick: () => void, props: Props = {}): HTMLElement {
  return el("button", { type: "button", ...props, onClick: () => onClick() }, label);
}

/**
 * Re-renders `root` with `render` and keeps the caret in a text field that had
 * it, matched by its aria-label, so a state push while typing loses nothing.
 */
export function keepingFocus(root: HTMLElement, render: () => void): void {
  const active = document.activeElement;
  const typed =
    (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) &&
    root.contains(active)
      ? active
      : null;
  const field = typed
    ? {
        label: typed.getAttribute("aria-label"),
        start: typed.selectionStart,
        end: typed.selectionEnd,
      }
    : null;
  render();
  if (!field?.label) return;
  for (const input of root.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>(
    "input, textarea",
  )) {
    if (input.getAttribute("aria-label") === field.label) {
      input.focus();
      if (field.start !== null && field.end !== null) {
        try {
          input.setSelectionRange(field.start, field.end);
        } catch {
          // Number inputs have no selection to restore.
        }
      }
      return;
    }
  }
}
