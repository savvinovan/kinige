// Finding elements by their accessible name, for the screens driver (KINIG-92, ADR-0016). Pure DOM reading, no events: `driver.ts` acts on
// what this finds. It is a small version of what `ax.js` did with the macOS accessibility tree ("a substring of the title, description or
// value; the optional role"), so the scenarios keep their words.

/** What to look for: `name` is matched against every accessible name of an element; `role` and `nth` narrow and pick. */
export interface Query {
  name: string;
  /** An ARIA role, lower case (`button`, `slider`, `menuitem`), implicit roles included. */
  role?: string;
  /** The zero-based place among the matches, in the order `findAll` returns them. */
  nth?: number;
}

/** Roles of things a person can operate; `findAll` puts them before plain text that happens to have the same words. */
const INTERACTIVE_ROLES = new Set([
  "button",
  "link",
  "checkbox",
  "radio",
  "switch",
  "tab",
  "menuitem",
  "menuitemcheckbox",
  "menuitemradio",
  "option",
  "slider",
  "spinbutton",
  "textbox",
  "searchbox",
  "combobox",
  "treeitem",
]);

const NEVER_NAMED = new Set(["SCRIPT", "STYLE", "TEMPLATE", "NOSCRIPT", "HEAD", "META", "LINK", "TITLE"]);

const BUTTON_INPUTS = new Set(["button", "submit", "reset", "image"]);

/** The explicit role, or the implicit one of the tag; empty when the element has none that matters here. */
export function roleOf(element: Element): string {
  const explicit = element.getAttribute("role")?.trim().split(/\s+/)[0];
  if (explicit) return explicit.toLowerCase();
  switch (element.tagName) {
    case "BUTTON":
    case "SUMMARY":
      return "button";
    case "A":
      return element.hasAttribute("href") ? "link" : "";
    case "TEXTAREA":
      return "textbox";
    case "SELECT":
      return "combobox";
    case "INPUT": {
      const type = (element.getAttribute("type") ?? "text").toLowerCase();
      if (type === "checkbox" || type === "radio") return type;
      if (type === "range") return "slider";
      if (type === "number") return "spinbutton";
      if (type === "search") return "searchbox";
      if (BUTTON_INPUTS.has(type)) return "button";
      return type === "hidden" ? "" : "textbox";
    }
    default:
      return element.getAttribute("contenteditable") === "true" || (element as HTMLElement).isContentEditable ? "textbox" : "";
  }
}

export function isInteractive(element: Element): boolean {
  return INTERACTIVE_ROLES.has(roleOf(element)) || element.hasAttribute("onclick");
}

/** Every accessible name an element answers to: its label, labelling elements, form label, title, value text and content. */
export function namesOf(element: Element): string[] {
  const names: string[] = [];
  const add = (text: string | null | undefined) => {
    const clean = squash(text);
    if (clean) names.push(clean);
  };
  for (const id of (element.getAttribute("aria-labelledby") ?? "").split(/\s+/)) {
    if (id) add(element.ownerDocument.getElementById(id)?.textContent);
  }
  add(element.getAttribute("aria-label"));
  const labels = (element as HTMLInputElement).labels;
  if (labels) for (const label of Array.from(labels)) add(label.textContent);
  add(element.getAttribute("title"));
  add(element.getAttribute("alt"));
  add(element.getAttribute("aria-valuetext"));
  add(element.getAttribute("aria-description"));
  add(element.getAttribute("placeholder"));
  if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) add(element.value);
  // Content: all of it for something operable (a button's text), only the element's own text nodes for anything else, so a container
  // does not answer to the words of everything inside it.
  if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement)) {
    add(isInteractive(element) ? element.textContent : ownText(element));
  }
  return names;
}

/** Not hidden by `hidden`, `aria-hidden`, `display: none` or `visibility: hidden` on the element or above it. */
export function isVisible(element: Element): boolean {
  const view = element.ownerDocument.defaultView;
  if (!view) return false;
  for (let node: Element | null = element; node; node = node.parentElement) {
    if (node.hasAttribute("hidden") || node.getAttribute("aria-hidden") === "true") return false;
    if (view.getComputedStyle(node).display === "none") return false;
  }
  const visibility = view.getComputedStyle(element).visibility;
  return visibility !== "hidden" && visibility !== "collapse";
}

export function isDisabled(element: Element): boolean {
  return (element as HTMLButtonElement).disabled === true || element.getAttribute("aria-disabled") === "true";
}

/**
 * The visible elements that answer to the query. Order: an exact name before one that only contains the words, an operable element
 * before plain text, then document order. `accept` narrows further (the targets of `type` are text fields only).
 */
export function findAll(root: ParentNode, query: Query, accept?: (element: Element) => boolean): Element[] {
  const wanted = squash(query.name);
  const role = query.role?.toLowerCase();
  const found: { element: Element; rank: number; order: number }[] = [];
  let order = 0;
  for (const element of Array.from(root.querySelectorAll("*"))) {
    order += 1;
    if (NEVER_NAMED.has(element.tagName)) continue;
    if (role && roleOf(element) !== role) continue;
    if (accept && !accept(element)) continue;
    const names = namesOf(element);
    const exact = names.some((name) => name === wanted);
    if (!exact && !names.some((name) => name.includes(wanted))) continue;
    if (!isVisible(element)) continue;
    found.push({ element, rank: (exact ? 0 : 2) + (isInteractive(element) ? 0 : 1), order });
  }
  found.sort((a, b) => a.rank - b.rank || a.order - b.order);
  return found.map((entry) => entry.element);
}

export function findOne(root: ParentNode, query: Query, accept?: (element: Element) => boolean): Element | undefined {
  return findAll(root, query, accept)[query.nth ?? 0];
}

/** Collapses runs of whitespace (including no-break spaces) and trims. */
export function squash(text: string | null | undefined): string {
  return (text ?? "").replace(/[\s ]+/g, " ").trim();
}

function ownText(element: Element): string {
  let text = "";
  for (const node of Array.from(element.childNodes)) {
    if (node.nodeType === Node.TEXT_NODE) text += node.nodeValue ?? "";
  }
  return text;
}
