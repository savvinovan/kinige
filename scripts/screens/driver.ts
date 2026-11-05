// The screens driver (KINIG-92, ADR-0016): the screens scenarios' hands inside the app's own web view. `web/vite.config.ts` points
// `virtual:kinige-driver` here only when the build is started with KINIGE_SCREENS_DRIVER=1; otherwise this file is not in the production
// module graph at all. A scenario script writes a step into a request file, the Rust side (cargo feature `screens-driver`) sends it to this
// window as an event, the code below finds the element by its accessible name and acts, and the answer goes back through the
// feature-gated `screens_driver_reply` command. It can do only what a person's hands can do in the window: no arbitrary script, no IPC
// passthrough. The elements are found by `driver-dom.ts`.
//
// The marker below is a string literal that survives minification. `build-app.sh` and `build-windows.sh` search `web/dist` (and the Rust
// binary) for it and refuse to build a bundle that has it unless KINIGE_ALLOW_SCREENS_DRIVER=1.
import { findAll, findOne, isDisabled, namesOf, roleOf, squash, type Query } from "./driver-dom";

export const DRIVER_MARKER = "kinige-screens-driver-92";
document.documentElement.dataset.screensDriver = DRIVER_MARKER;

const STEP_EVENT = "screens-driver-step";
const REPLY_COMMAND = "screens_driver_reply";
/** How long a step waits for its element to appear before it reports what was not found ("a second or so"). */
const PATIENCE_MS = 1000;
const POLL_MS = 50;

/** The steps of v1; the Rust side validates the same shapes (`src-tauri/src/screens_driver/request.rs`) before it sends one. */
export type Step =
  | ({ kind: "press" } & Query)
  | ({ kind: "menu" } & Query)
  | { kind: "type"; name?: string; text: string }
  | { kind: "key"; key: string; code?: string; mods?: string[] }
  | { kind: "wait"; name: string; role?: string; gone?: boolean; timeoutMs: number }
  | { kind: "has"; name: string; role?: string }
  | ({ kind: "scroll" } & Query)
  | { kind: "text" };

export type Answer = { ok: true; value?: string } | { ok: false; error: string };

export interface RunOptions {
  /** The document to act on; the real one unless a test brings its own. */
  root?: Document;
  /** How long to wait for an element; shortened by tests. */
  patienceMs?: number;
}

/** What this file needs from the page, passed in by `main.tsx` (this file cannot import the Tauri package from outside `web/`). */
export interface DriverHost {
  /** This window's label (`main`, `settings` or `capture`): a step addressed to another window is not ours to run. */
  window: string;
  listen: (event: string, handler: (event: { payload: unknown }) => void) => Promise<unknown>;
  invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>;
}

/**
 * Subscribes to the steps and says this window is ready; a step that arrives before this is not delivered. The subscription is an `Any`
 * listener, so tauri hands this page the steps meant for every window (`emit_to` reaches all `Any` listeners): the payload names its
 * window and `handle` drops the others. Without that a hidden window would run a step meant for `main`.
 */
export function startDriver(host: DriverHost): void {
  void host
    .listen(STEP_EVENT, (event) => void handle(host, event.payload))
    .then(() => host.invoke(REPLY_COMMAND, { message: { kind: "ready" } }))
    .catch(() => undefined);
}

async function handle(host: DriverHost, payload: unknown): Promise<void> {
  const request = payload as { id?: unknown; window?: unknown; step?: unknown } | null;
  if (typeof request?.id !== "string" || request.window !== host.window) return;
  const answer = await runStep(request.step);
  const message = { kind: "reply", id: request.id, ok: answer.ok, value: answer.ok ? answer.value : undefined, error: answer.ok ? undefined : answer.error };
  await host.invoke(REPLY_COMMAND, { message }).catch(() => undefined);
}

/** Runs one step; never throws, an error is an answer. */
export async function runStep(step: unknown, options: RunOptions = {}): Promise<Answer> {
  const root = options.root ?? document;
  const patience = options.patienceMs ?? PATIENCE_MS;
  try {
    const value = await run(step as Step, root, patience);
    return value === undefined ? { ok: true } : { ok: true, value };
  } catch (reason) {
    return { ok: false, error: reason instanceof Error ? reason.message : "the step failed" };
  }
}

async function run(step: Step, root: Document, patience: number): Promise<string | undefined> {
  switch (step?.kind) {
    case "press": {
      const element = await need(root, step, patience);
      press(element);
      return undefined;
    }
    case "menu": {
      const element = await need(root, step, patience);
      contextMenu(element);
      return undefined;
    }
    case "type":
      await typeInto(root, step, patience);
      return undefined;
    case "key":
      pressKey(root, step);
      return undefined;
    case "wait":
      await waitFor(root, step);
      return undefined;
    case "has":
      return findAll(root, step).length > 0 ? "yes" : "no";
    case "scroll": {
      const element = await need(root, step, patience);
      element.scrollIntoView?.({ block: "nearest", inline: "nearest" });
      return undefined;
    }
    case "text":
      return windowText(root);
    default:
      throw new Error(`unknown step: ${String((step as { kind?: unknown } | null)?.kind)}`);
  }
}

/** The element of the query, waiting up to `patience` for it to show; an error names what was not found. */
async function need(root: Document, query: Query, patience: number, accept?: (element: Element) => boolean): Promise<HTMLElement> {
  if (typeof query.name !== "string" || query.name === "") throw new Error("the step has no name");
  const deadline = Date.now() + patience;
  for (;;) {
    const element = findOne(root, query, accept);
    if (element) return element as HTMLElement;
    if (Date.now() >= deadline) throw new Error(`not found: ${describe(query)}`);
    await sleep(POLL_MS);
  }
}

function describe(query: Query): string {
  return `"${query.name}"${query.role ? ` (${query.role})` : ""}${query.nth ? ` #${query.nth}` : ""}`;
}

/**
 * A click as a hand makes it: scroll to it, pointer and mouse down and up at its centre, then the click. The scroll is the smallest one
 * (`nearest`): an element already on screen does not move the feed, so a shot taken after a press keeps its framing.
 */
function press(element: HTMLElement): void {
  if (isDisabled(element)) throw new Error(`disabled: ${namesOf(element)[0] ?? element.tagName}`);
  element.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  const point = centre(element);
  pointer(element, "pointerdown", point, 0, 1);
  mouse(element, "mousedown", point, 0, 1);
  try {
    element.focus({ preventScroll: true });
  } catch {
    // Not focusable: the click goes ahead anyway.
  }
  pointer(element, "pointerup", point, 0, 0);
  mouse(element, "mouseup", point, 0, 0);
  element.click();
}

/** A right click: the page gets `contextmenu` at the element's centre. */
function contextMenu(element: HTMLElement): void {
  const point = centre(element);
  mouse(element, "contextmenu", point, 2, 2);
}

function centre(element: HTMLElement): { x: number; y: number } {
  const box = element.getBoundingClientRect();
  return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
}

function mouse(target: Element, type: string, point: { x: number; y: number }, button: number, buttons: number): void {
  target.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, composed: true, clientX: point.x, clientY: point.y, button, buttons }));
}

function pointer(target: Element, type: string, point: { x: number; y: number }, button: number, buttons: number): void {
  // A web view without PointerEvent falls back to the mouse event of the same name.
  if (typeof PointerEvent === "undefined") return mouse(target, type, point, button, buttons);
  const init = { bubbles: true, cancelable: true, composed: true, clientX: point.x, clientY: point.y, button, buttons };
  target.dispatchEvent(new PointerEvent(type, { ...init, pointerId: 1, pointerType: "mouse", isPrimary: true }));
}

/** Puts `text` into a text field (replacing what is there): the named one or the focused one. React sees it as typing. */
async function typeInto(root: Document, step: { name?: string; text: string }, patience: number): Promise<void> {
  if (typeof step.text !== "string") throw new Error("the type step has no text");
  const field = step.name ? await need(root, { name: step.name }, patience, isTextField) : root.activeElement;
  if (!(field instanceof HTMLElement) || !isTextField(field)) throw new Error("no text field to type into");
  field.focus({ preventScroll: true });
  if (field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement) {
    // The native setter, not `field.value = ...`: React tracks the value property it wrapped and would drop the change.
    const prototype = field instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    const set = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
    if (!set) throw new Error("the field has no value setter");
    set.call(field, step.text);
    field.dispatchEvent(new Event("input", { bubbles: true }));
    return;
  }
  // A contenteditable (the composer, if it is one): select what is there and insert, so the editor's own handlers run.
  window.getSelection()?.selectAllChildren(field);
  if (!root.execCommand?.("insertText", false, step.text)) {
    field.textContent = step.text;
    field.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: step.text }));
  }
}

const NOT_TEXT_INPUTS = ["checkbox", "radio", "button", "submit", "reset", "range", "image", "file", "hidden"];

function isTextField(element: Element): boolean {
  if (element instanceof HTMLTextAreaElement) return true;
  if (element instanceof HTMLInputElement) return !NOT_TEXT_INPUTS.includes(element.type);
  return ["textbox", "searchbox"].includes(roleOf(element));
}

/** keydown and keyup on the focused element. A script-made key does not do what the browser does for a real one (Enter on a button). */
function pressKey(root: Document, step: { key: string; code?: string; mods?: string[] }): void {
  if (typeof step.key !== "string" || step.key === "") throw new Error("the key step has no key");
  const target = root.activeElement ?? root.body;
  const mods = new Set(step.mods ?? []);
  const init = { key: step.key, code: step.code ?? "", bubbles: true, cancelable: true, composed: true, ctrlKey: mods.has("ctrl"), shiftKey: mods.has("shift"), altKey: mods.has("alt"), metaKey: mods.has("meta") };
  target.dispatchEvent(new KeyboardEvent("keydown", init));
  target.dispatchEvent(new KeyboardEvent("keyup", init));
}

/** Waits (polling) until the element shows, or until it is gone with `gone`; the error says which. */
async function waitFor(root: Document, step: { name: string; role?: string; gone?: boolean; timeoutMs: number }): Promise<void> {
  if (typeof step.name !== "string" || step.name === "") throw new Error("the step has no name");
  if (typeof step.timeoutMs !== "number" || !Number.isFinite(step.timeoutMs)) throw new Error("the wait step has no timeoutMs");
  const limit = Math.max(0, step.timeoutMs);
  const deadline = Date.now() + limit;
  for (;;) {
    const present = findAll(root, step).length > 0;
    if (present !== Boolean(step.gone)) return;
    if (Date.now() >= deadline) throw new Error(`${step.gone ? "still there" : "not found"} after ${limit} ms: ${describe(step)}`);
    await sleep(POLL_MS);
  }
}

/** What the window shows and says: the rendered text, every label and title, the text typed into fields. For the personal-path check. */
function windowText(root: Document): string {
  const body = root.body;
  const parts: string[] = [];
  for (const element of Array.from(body.querySelectorAll("[aria-label], [title], [aria-valuetext], [placeholder], [alt], input, textarea"))) {
    for (const attribute of ["aria-label", "title", "aria-valuetext", "placeholder", "alt"]) parts.push(squash(element.getAttribute(attribute)));
    if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) parts.push(squash(element.value));
  }
  return [body.innerText ?? body.textContent ?? "", ...parts].filter(Boolean).join("\n");
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
