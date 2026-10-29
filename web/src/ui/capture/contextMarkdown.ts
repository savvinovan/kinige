import { renderMarkdown, type TaskLabels } from "../message/markdown";

/**
 * Markdown записи для окна захвата: тот же безопасный рендер, что и в ленте, но без всего, что можно нажать.
 * У окна захвата нет права открывать ссылки (`open_external_url` ему не разрешена), поэтому ссылка становится обычным
 * текстом (`<span class="capture-link">` без адреса), а всё, что берёт фокус (теги, флажки задач), теряет `tabindex`:
 * запись контекста — подсказка, а не элемент управления.
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
