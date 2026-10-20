import createDOMPurify from "dompurify";
import MarkdownIt from "markdown-it";

// Безопасный рендер Markdown в ленте (ТЗ §6): сырой HTML в тексте отключён (он выводится как текст), картинки и таблицы
// в D1 не рендерятся, ссылки только http(s) и mailto, результат дополнительно проходит DOMPurify по белому списку.
// Заметки — пользовательские данные, а у webview есть доступ к командам Tauri, поэтому защита двойная.

type RenderEnv = { timeSpacer?: boolean };

const md = new MarkdownIt({ html: false, linkify: true, breaks: true, typographer: false });
md.linkify.set({ fuzzyLink: false, fuzzyEmail: false });
md.disable(["image", "table"]);

// Блок кода: без подсветки синтаксиса, фокусируется с клавиатуры (длинные строки прокручиваются по горизонтали).
md.renderer.rules.fence = (tokens, idx) =>
  `<pre class="code-block" tabindex="0"><code>${md.utils.escapeHtml(tokens[idx]?.content ?? "")}</code></pre>\n`;

// Ссылка остаётся настоящим <a href> (видна диктору, берёт фокус, адрес читается при наведении), но в D1 переход по ней
// отменяется в Message: webview не должен уходить с экрана приложения на внешний сайт. Открытие в системном браузере —
// слайс 1 (команда или плагин Tauri и capability); пока его нет, щелчок и Enter по ссылке ничего не открывают.
// rel закрывает доступ к window.opener на случай, если переход когда-нибудь разрешат.
const defaultLinkOpen = md.renderer.rules.link_open ?? ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
md.renderer.rules.link_open = (tokens, idx, options, env, self) => {
  tokens[idx]?.attrSet("rel", "noopener noreferrer");
  return defaultLinkOpen(tokens, idx, options, env, self);
};

// Задачи `- [ ] текст` и `- [x] текст`: пункт получает маркер-квадрат, список — класс task-list.
md.core.ruler.after("inline", "kinige-task-lists", (state) => {
  const { tokens } = state;
  for (let i = 2; i < tokens.length; i++) {
    const inline = tokens[i];
    const paragraph = tokens[i - 1];
    const item = tokens[i - 2];
    if (inline?.type !== "inline" || paragraph?.type !== "paragraph_open" || item?.type !== "list_item_open") continue;
    const match = /^\[([ xX])\][ \t]+/.exec(inline.content);
    if (!match) continue;

    const done = match[1] !== " ";
    inline.content = inline.content.slice(match[0].length);
    const first = inline.children?.[0];
    if (first?.type === "text") first.content = first.content.replace(/^\[[ xX]\][ \t]+/, "");

    const box = new state.Token("html_inline", "", 0);
    box.content = `<span class="task__box" role="img" aria-label="${done ? "Выполнено" : "Не выполнено"}"></span>`;
    inline.children?.unshift(box);

    item.attrJoin("class", done ? "task task--done" : "task");
    for (let j = i - 3; j >= 0; j--) {
      const list = tokens[j];
      if (list && list.level === item.level - 1 && list.type.endsWith("_list_open")) {
        if (!String(list.attrGet("class") ?? "").includes("task-list")) list.attrJoin("class", "task-list");
        break;
      }
    }
  }
});

// Время стоит в правом нижнем углу плашки на одной строке с концом текста: в конец последней строки вставляется
// невидимая распорка по ширине времени, чтобы текст его не накрывал. Если сообщение кончается блоком кода,
// распорки нет: время стоит под блоком.
md.core.ruler.push("kinige-time-spacer", (state) => {
  if (!(state.env as RenderEnv).timeSpacer) return;
  let last = state.tokens.length - 1;
  while (last >= 0 && state.tokens[last]?.type.endsWith("_close")) last--;
  const inline = state.tokens[last];
  if (inline?.type !== "inline") return;
  const spacer = new state.Token("html_inline", "", 0);
  spacer.content = '<span class="message__spacer" aria-hidden="true"></span>';
  inline.children?.push(spacer);
});

const purify = createDOMPurify(window);
// Адреса: только http, https и mailto (по умолчанию DOMPurify пропускает ещё tel:, sms:, относительные и другие).
// Свой ALLOWED_URI_REGEXP здесь нельзя: DOMPurify применяет его ко всем атрибутам вне списка «безопасных для URI»
// и вычистил бы tabindex и aria-label, поэтому адрес проверяется отдельно.
const SAFE_HREF = /^(?:https?:|mailto:)/i;
purify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName !== "A") return;
  const href = node.getAttribute("href");
  if (href !== null && !SAFE_HREF.test(href.trim())) node.removeAttribute("href");
  node.setAttribute("rel", "noopener noreferrer");
});

const SANITIZE = {
  ALLOWED_TAGS: [
    "p", "br", "hr", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "blockquote", "pre", "code",
    "em", "strong", "s", "a", "span",
  ],
  ALLOWED_ATTR: ["href", "rel", "class", "role", "aria-label", "aria-hidden", "tabindex", "start"],
  ALLOW_DATA_ATTR: false,
  ALLOW_ARIA_ATTR: false,
};

/** Второй рубеж защиты: белый список тегов, атрибутов и адресов. Отдельно экспортируется ради теста. */
export function sanitizeHtml(html: string): string {
  return purify.sanitize(html, SANITIZE);
}

/**
 * Markdown сообщения -> безопасный HTML для плашки.
 * @param timeSpacer вставить в конец последней строки распорку под время (нет при состоянии «не отправилось»,
 * когда время стоит в нижней строке рядом с «Повторить»).
 */
export function renderMarkdown(source: string, { timeSpacer }: { timeSpacer: boolean }): string {
  return sanitizeHtml(md.render(source, { timeSpacer } satisfies RenderEnv));
}
