import createDOMPurify from "dompurify";
import MarkdownIt, { type StateCore, type Token } from "markdown-it";

// Безопасный рендер Markdown в ленте (ТЗ §6): сырой HTML в тексте отключён (он выводится как текст), картинки и таблицы
// в D1 не рендерятся, ссылки только http(s) и mailto, результат дополнительно проходит DOMPurify по белому списку.
// Заметки — пользовательские данные, а у webview есть доступ к командам Tauri, поэтому защита двойная.

/** Подписи квадратиков задач для диктора: из переводов, поэтому приходят снаружи. */
export interface TaskLabels {
  done: string;
  todo: string;
  /**
   * Имя кликабельного квадрата (`interactiveTasks`): у флажка имя не должно зависеть от состояния, состояние говорит
   * `aria-checked`. Без него берутся `done` / `todo`.
   */
  item?: string;
}

type RenderEnv = {
  timeSpacer?: boolean;
  taskLabels?: TaskLabels;
  tags?: ReadonlySet<string>;
  interactiveTasks?: boolean;
};

const md = new MarkdownIt({ html: false, linkify: true, breaks: true, typographer: false });
md.linkify.set({ fuzzyLink: false, fuzzyEmail: false });
md.disable(["image", "table"]);

// Блок кода: без подсветки синтаксиса, фокусируется с клавиатуры (длинные строки прокручиваются по горизонтали).
md.renderer.rules.fence = (tokens, idx) =>
  `<pre class="code-block" tabindex="0"><code>${md.utils.escapeHtml(tokens[idx]?.content ?? "")}</code></pre>\n`;

// Ссылка остаётся настоящим <a href> (видна диктору, берёт фокус, адрес читается при наведении), но переход по ней
// отменяется в Message: webview не должен уходить с экрана приложения на внешний сайт. Адрес уходит в системный браузер
// командой `open_external_url` (слайс 1), а «Open Link» из системного меню закрывает `on_navigation` в Rust.
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
    const { taskLabels: labels, interactiveTasks } = state.env as RenderEnv;
    if (interactiveTasks) {
      // Кликабельный квадрат (слайс 3): флажок с клавиатуры; щелчок и пробел обрабатывает Message по порядковому номеру.
      const label = md.utils.escapeHtml(labels?.item ?? (done ? labels?.done : labels?.todo) ?? "");
      box.content = `<span class="task__box" role="checkbox" aria-checked="${done}" aria-label="${label}" tabindex="0"></span>`;
    } else {
      const label = md.utils.escapeHtml((done ? labels?.done : labels?.todo) ?? "");
      box.content = `<span class="task__box" role="img" aria-label="${label}"></span>`;
    }
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

// Теги `#слово` (слайс 3). Какие слова — теги, решает Rust (`kinige_domain::tags`), сюда они приходят списком в `env.tags`
// (нижний регистр, без `#`); здесь только подсветка. Правило идёт после linkify и до text_join: ссылки уже видны как
// пары link_open / link_close (внутри них тегов нет), а экранированное `\#` ещё отдельный токен text_special.
// Код (`code_inline`, блоки) в текстовые токены не попадает. Слово — буквы любого алфавита, цифры и `_`; перед `#`
// нет буквы, цифры, `_`, `#`, `&` и `\`.
const TAG_AT = /#([\p{L}\p{N}_]+)/gu;
const NOT_BEFORE_TAG = /[\p{L}\p{N}_#&\\]$/u;

/** Текстовый токен → цепочка из текста и `<span class="tag">`; без тегов возвращает `null`. */
function splitTags(state: StateCore, token: Token, before: string, tags: ReadonlySet<string>): Token[] | null {
  const text = token.content;
  const parts: Token[] = [];
  let from = 0;
  for (const match of text.matchAll(TAG_AT)) {
    const word = match[1] ?? "";
    const preceding = match.index === 0 ? before : text.slice(0, match.index);
    if (NOT_BEFORE_TAG.test(preceding) || !tags.has(word.toLowerCase())) continue;
    if (match.index > from) {
      const plain = new state.Token("text", "", 0);
      plain.content = text.slice(from, match.index);
      parts.push(plain);
    }
    const tag = new state.Token("html_inline", "", 0);
    tag.content = `<span class="tag" role="button" tabindex="0">#${md.utils.escapeHtml(word)}</span>`;
    parts.push(tag);
    from = match.index + match[0].length;
  }
  if (parts.length === 0) return null;
  if (from < text.length) {
    const rest = new state.Token("text", "", 0);
    rest.content = text.slice(from);
    parts.push(rest);
  }
  return parts;
}

md.core.ruler.after("linkify", "kinige-tags", (state) => {
  const tags = (state.env as RenderEnv).tags;
  if (!tags || tags.size === 0) return;
  for (const block of state.tokens) {
    if (block.type !== "inline" || !block.children) continue;
    const result: Token[] = [];
    let linkDepth = 0;
    let before = "";
    for (const token of block.children) {
      if (token.type === "link_open") linkDepth++;
      else if (token.type === "link_close") linkDepth--;
      const parts = token.type === "text" && linkDepth === 0 ? splitTags(state, token, before, tags) : null;
      result.push(...(parts ?? [token]));
      // Что стоит перед следующим токеном: последний знак соседнего текста; всё остальное (разметка, код) — «ничего».
      before = token.type === "text" || token.type === "text_special" ? token.content : "";
    }
    block.children = result;
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
  ALLOWED_ATTR: ["href", "rel", "class", "role", "aria-label", "aria-hidden", "aria-checked", "tabindex", "start"],
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
 * @param taskLabels подписи квадратиков задач (переводы).
 * @param tags теги записи (нижний регистр, без `#`), которые нужно подсветить в тексте; их определил Rust.
 * @param interactiveTasks квадраты `- [ ]` становятся флажками (`role="checkbox"`, `aria-checked`, `tabindex="0"`).
 */
export function renderMarkdown(
  source: string,
  {
    timeSpacer,
    taskLabels,
    tags,
    interactiveTasks,
  }: { timeSpacer: boolean; taskLabels: TaskLabels; tags?: readonly string[]; interactiveTasks?: boolean },
): string {
  const env: RenderEnv = { timeSpacer, taskLabels, tags: tags ? new Set(tags) : undefined, interactiveTasks };
  return sanitizeHtml(md.render(source, env));
}
