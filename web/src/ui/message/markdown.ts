import createDOMPurify from "dompurify";
import MarkdownIt, { type StateCore, type Token } from "markdown-it";

// Safe Markdown rendering in the feed (spec §6): raw HTML in the text is disabled (it is shown as text), images and tables
// are not rendered in D1, links are only http(s) and mailto, and the result additionally goes through DOMPurify with an allow list.
// Notes are user data, and the webview has access to Tauri commands, so the protection is double.

/** Task box labels for the screen reader: from the translations, so they come from outside. */
export interface TaskLabels {
  done: string;
  todo: string;
  /**
   * Name of the clickable box (`interactiveTasks`): a checkbox's name must not depend on its state, the state is conveyed
   * by `aria-checked`. Without it `done` / `todo` are used.
   */
  item?: string;
}

type RenderEnv = {
  timeSpacer?: boolean;
  taskLabels?: TaskLabels;
  tags?: ReadonlySet<string>;
  interactiveTasks?: boolean;
  /** Words found by search (as written in the entry): highlighted in plain text as whole words. */
  marks?: ReadonlySet<string>;
};

const md = new MarkdownIt({ html: false, linkify: true, breaks: true, typographer: false });
md.linkify.set({ fuzzyLink: false, fuzzyEmail: false });
md.disable(["image", "table"]);

// Code block: no syntax highlighting; long lines wrap (markdown.css), there is no scrolling, so focus is not needed either.
md.renderer.rules.fence = (tokens, idx) =>
  `<pre class="code-block"><code>${md.utils.escapeHtml(tokens[idx]?.content ?? "")}</code></pre>\n`;

// The link stays a real <a href> (visible to the screen reader, takes focus, the address is readable on hover), but following it
// is cancelled in Message: the webview must not leave the app screen for an external site. The address goes to the system browser
// via the `open_external_url` command (slice 1), and "Open Link" from the system menu is blocked by `on_navigation` in Rust.
// rel closes access to window.opener in case navigation is ever allowed.
const defaultLinkOpen = md.renderer.rules.link_open ?? ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
md.renderer.rules.link_open = (tokens, idx, options, env, self) => {
  tokens[idx]?.attrSet("rel", "noopener noreferrer");
  return defaultLinkOpen(tokens, idx, options, env, self);
};

// Tasks `- [ ] text` and `- [x] text`: the item gets a box marker, the list gets the task-list class.
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
      // Clickable box (slice 3): a checkbox from the keyboard; Message handles click and Space by the ordinal number.
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

// Tags `#word` (slice 3). Which words are tags is decided by Rust (`kinige_domain::tags`), they arrive here as a list in `env.tags`
// (lower case, without `#`); only highlighting is done here. The rule runs after linkify and before text_join: links are already visible as
// link_open / link_close pairs (there are no tags inside them), and the escaped `\#` is still a separate text_special token.
// Code (`code_inline`, blocks) does not get into text tokens. A word is letters of any alphabet, digits and `_`; before `#`
// there is no letter, digit, `_`, `#`, `&` or `\`.
const TAG_AT = /#([\p{L}\p{N}_]+)/gu;
const NOT_BEFORE_TAG = /[\p{L}\p{N}_#&\\]$/u;

/** Text token -> a chain of text and `<span class="tag">`; returns `null` if there are no tags. */
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
      // What stands before the next token: the last character of the neighboring text; everything else (markup, code) is "nothing".
      before = token.type === "text" || token.type === "text_special" ? token.content : "";
    }
    block.children = result;
  }
});

// Words found by search (slice 4): the entry's words returned by Rust are wrapped in `<mark>` in plain text. Only whole words match,
// with the same case as the word is written in the entry; there is no highlighting in links, code and tags (after
// "kinige-tags" the tags are already html_inline tokens, the text of links is skipped by depth).
const MARK_WORD = /[\p{L}\p{N}]+/gu;

function splitMarks(state: StateCore, token: Token, marks: ReadonlySet<string>): Token[] | null {
  const text = token.content;
  const parts: Token[] = [];
  let from = 0;
  for (const match of text.matchAll(MARK_WORD)) {
    if (!marks.has(match[0])) continue;
    if (match.index > from) {
      const plain = new state.Token("text", "", 0);
      plain.content = text.slice(from, match.index);
      parts.push(plain);
    }
    const mark = new state.Token("html_inline", "", 0);
    mark.content = `<mark>${md.utils.escapeHtml(match[0])}</mark>`;
    parts.push(mark);
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

md.core.ruler.after("kinige-tags", "kinige-marks", (state) => {
  const marks = (state.env as RenderEnv).marks;
  if (!marks || marks.size === 0) return;
  for (const block of state.tokens) {
    if (block.type !== "inline" || !block.children) continue;
    const result: Token[] = [];
    let linkDepth = 0;
    for (const token of block.children) {
      if (token.type === "link_open") linkDepth++;
      else if (token.type === "link_close") linkDepth--;
      const parts = token.type === "text" && linkDepth === 0 ? splitMarks(state, token, marks) : null;
      result.push(...(parts ?? [token]));
    }
    block.children = result;
  }
});

// The time sits in the bottom right corner of the bubble on the same line as the end of the text: an invisible spacer the width of the time
// is inserted at the end of the last line so that the text does not cover it. If the message ends with a code block,
// there is no spacer: the time sits under the block.
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
// Addresses: only http, https and mailto (by default DOMPurify also lets through tel:, sms:, relative ones and others).
// A custom ALLOWED_URI_REGEXP is not an option here: DOMPurify applies it to all attributes outside the "URI-safe" list
// and would strip tabindex and aria-label, so the address is checked separately.
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
    "em", "strong", "s", "a", "span", "mark",
  ],
  ALLOWED_ATTR: ["href", "rel", "class", "role", "aria-label", "aria-hidden", "aria-checked", "tabindex", "start"],
  ALLOW_DATA_ATTR: false,
  ALLOW_ARIA_ATTR: false,
};

/** The second line of defense: an allow list of tags, attributes and addresses. Exported separately for the test. */
export function sanitizeHtml(html: string): string {
  return purify.sanitize(html, SANITIZE);
}

/**
 * Message Markdown -> safe HTML for the bubble.
 * @param timeSpacer insert a spacer for the time at the end of the last line (not in the "not sent" state,
 * when the time sits in the bottom line next to "Retry").
 * @param taskLabels task box labels (translations).
 * @param tags entry tags (lower case, without `#`) to highlight in the text; determined by Rust.
 * @param interactiveTasks `- [ ]` boxes become checkboxes (`role="checkbox"`, `aria-checked`, `tabindex="0"`).
 */
export function renderMarkdown(
  source: string,
  {
    timeSpacer,
    taskLabels,
    tags,
    interactiveTasks,
    marks,
  }: {
    timeSpacer: boolean;
    taskLabels: TaskLabels;
    tags?: readonly string[];
    interactiveTasks?: boolean;
    marks?: readonly string[];
  },
): string {
  const env: RenderEnv = {
    timeSpacer,
    taskLabels,
    tags: tags ? new Set(tags) : undefined,
    interactiveTasks,
    marks: marks ? new Set(marks) : undefined,
  };
  return sanitizeHtml(md.render(source, env));
}
