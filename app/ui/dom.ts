// A tiny element builder. Text always goes in through `textContent`, never
// `innerHTML`, so a device name can't inject markup.

type Child = Node | string | null | undefined | false;

export interface Props {
  class?: string;
  title?: string;
  disabled?: boolean;
  type?: string;
  value?: string;
  ariaLabel?: string;
  onClick?: (event: MouseEvent) => void;
  onKey?: (event: KeyboardEvent) => void;
  onInput?: (value: string) => void;
}

export function el(tag: string, props: Props = {}, ...children: Child[]): HTMLElement {
  const node = document.createElement(tag);
  if (props.class) node.className = props.class;
  if (props.title) node.title = props.title;
  if (props.ariaLabel) node.setAttribute("aria-label", props.ariaLabel);
  if (props.type) node.setAttribute("type", props.type);
  if (props.disabled) node.setAttribute("disabled", "");
  if (props.value !== undefined && node instanceof HTMLInputElement) node.value = props.value;
  if (props.onClick) node.addEventListener("click", props.onClick);
  if (props.onKey) node.addEventListener("keydown", props.onKey);
  const onInput = props.onInput;
  if (onInput && node instanceof HTMLInputElement) {
    node.addEventListener("input", () => onInput(node.value));
  }
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    node.append(typeof child === "string" ? document.createTextNode(child) : child);
  }
  return node;
}

export function button(label: string, onClick: () => void, props: Props = {}): HTMLElement {
  return el("button", { type: "button", ...props, onClick: () => onClick() }, label);
}
