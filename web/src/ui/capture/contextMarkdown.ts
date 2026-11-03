import { renderMarkdown, type TaskLabels } from "../message/markdown";

/**
 * An entry's Markdown for the capture window: the same safe rendering as in the feed, but without anything clickable.
 * The capture window has no right to open links (`open_external_url` is not allowed for it), so a link becomes plain
 * text (`<span class="capture-link">` without an address), and everything that takes focus (tags, task checkboxes) loses `tabindex`:
 * a context entry is a hint, not a control.
 */
export function renderContextMarkdown(source: string, taskLabels: TaskLabels): string {
  const template = document.createElement("template");
  template.innerHTML = renderMarkdown(source, { timeSpacer: false, taskLabels });
  for (const link of template.content.querySelectorAll("a")) {
    const text = document.createElement("span");
    text.className = "capture-link";
    text.append(...link.childNodes);
    link.replaceWith(text);
  }
  for (const element of template.content.querySelectorAll("[tabindex]")) element.removeAttribute("tabindex");
  return template.innerHTML;
}
