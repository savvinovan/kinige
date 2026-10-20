import { describe, expect, it } from "vitest";
import { renderMarkdown, sanitizeHtml } from "./markdown";

const render = (source: string, timeSpacer = false) => {
  const host = document.createElement("div");
  host.innerHTML = renderMarkdown(source, { timeSpacer });
  return host;
};

describe("renderMarkdown: разметка", () => {
  it("курсив, жирный, строчный код, перенос строки как в мессенджере", () => {
    const host = render("*раз* и **два**, `git rebase`\nвторая строка");
    expect(host.querySelector("em")?.textContent).toBe("раз");
    expect(host.querySelector("strong")?.textContent).toBe("два");
    expect(host.querySelector("code")?.textContent).toBe("git rebase");
    expect(host.querySelector("br")).not.toBeNull();
  });

  it("заголовки H1–H3", () => {
    const host = render("# Один\n\n## Два\n\n### Три");
    expect([...host.querySelectorAll("h1, h2, h3")].map((h) => `${h.tagName}:${h.textContent}`)).toEqual([
      "H1:Один",
      "H2:Два",
      "H3:Три",
    ]);
  });

  it("маркированный и нумерованный списки", () => {
    const host = render("- яблоко\n- груша\n\n1. раз\n2. два");
    expect(host.querySelectorAll("ul > li")).toHaveLength(2);
    expect(host.querySelectorAll("ol > li")).toHaveLength(2);
  });

  it("задачи: маркер-квадрат, состояние в подписи для диктора, текст без «[ ]»", () => {
    const host = render("- [x] Поход в горы\n- [ ] Разобрать фотографии");
    const items = [...host.querySelectorAll("li")];
    expect(items.map((li) => li.textContent)).toEqual(["Поход в горы", "Разобрать фотографии"]);
    expect(items[0]?.className).toContain("task--done");
    expect(items[1]?.className).not.toContain("task--done");
    expect(items.map((li) => li.querySelector(".task__box")?.getAttribute("aria-label"))).toEqual(["Выполнено", "Не выполнено"]);
    expect(host.querySelector("ul")?.className).toBe("task-list");
  });

  it("обычный пункт с квадратными скобками не становится задачей", () => {
    const host = render("- [важно] не задача\n- просто пункт");
    expect(host.querySelector(".task__box")).toBeNull();
    expect(host.querySelector("ul")?.className).toBe("");
  });

  it("цитата: черта только у цитаты, следующий абзац вне blockquote", () => {
    const host = render("> Лучше сделано, чем идеально.\n\nЗаписал, чтобы перечитать.");
    expect(host.querySelector("blockquote")?.textContent?.trim()).toBe("Лучше сделано, чем идеально.");
    const paragraphs = [...host.querySelectorAll("p")];
    expect(paragraphs.at(-1)?.closest("blockquote")).toBeNull();
    expect(paragraphs.at(-1)?.textContent).toBe("Записал, чтобы перечитать.");
  });

  it("блок кода: текст сохраняется как есть, фокусируется с клавиатуры, внутри нет разметки", () => {
    const host = render("```rust\nfn main() {\n    println!(\"<b>\");\n}\n```");
    const pre = host.querySelector("pre");
    expect(pre?.getAttribute("tabindex")).toBe("0");
    expect(pre?.textContent).toBe('fn main() {\n    println!("<b>");\n}\n');
    expect(pre?.querySelector("b")).toBeNull();
  });

  it("ссылка: rel закрывает window.opener", () => {
    const host = render("[fs.blog](https://fs.blog/habits)");
    const link = host.querySelector("a");
    expect(link?.getAttribute("href")).toBe("https://fs.blog/habits");
    expect(link?.getAttribute("rel")).toBe("noopener noreferrer");
  });

  it("адрес с явной схемой становится ссылкой, «голое» слово с точкой — нет", () => {
    const host = render("смотри https://example.com/a и example.com");
    expect([...host.querySelectorAll("a")].map((a) => a.getAttribute("href"))).toEqual(["https://example.com/a"]);
  });
});

describe("renderMarkdown: безопасность (ТЗ §6)", () => {
  it("сырой HTML выводится текстом, а не разметкой", () => {
    const host = render('<script>alert(1)</script> <img src=x onerror=alert(1)> <b onclick="x()">жирный?</b>');
    expect(host.querySelector("script, img, b")).toBeNull();
    expect(host.textContent).toContain("<script>alert(1)</script>");
    expect(host.innerHTML).not.toContain("<img");
    expect(host.innerHTML).not.toContain("<script");
  });

  it("ссылки с javascript:, data: и vbscript: не становятся ссылками", () => {
    const host = render("[a](javascript:alert(1)) [b](data:text/html,<script>1</script>) [c](vbscript:x) [d](file:///etc/passwd)");
    expect(host.querySelector("a")).toBeNull();
  });

  it("картинки и таблицы в D1 не рендерятся", () => {
    const host = render("![лес](https://example.com/a.png)\n\n| a | b |\n|---|---|\n| 1 | 2 |");
    expect(host.querySelector("img, table")).toBeNull();
  });

  it("второй рубеж: DOMPurify вычищает всё, чего нет в белом списке", () => {
    const dirty =
      '<p onclick="x()" style="color:red" data-x="1">текст</p><script>alert(1)</script><iframe src="https://e.com"></iframe>' +
      '<img src=x onerror=alert(1)><a href="javascript:alert(1)">a</a><a href="https://ok.example/">ok</a>' +
      '<svg onload=alert(1)></svg><form action="https://e.com"><input name="x"></form><style>p{color:red}</style>';
    const host = document.createElement("div");
    host.innerHTML = sanitizeHtml(dirty);
    expect(host.querySelector("script, iframe, img, svg, form, input, style")).toBeNull();
    expect(host.querySelector("p")?.getAttributeNames()).toEqual([]);
    expect([...host.querySelectorAll("a")].map((a) => a.getAttribute("href"))).toEqual([null, "https://ok.example/"]);
    expect(host.querySelector('a[href="https://ok.example/"]')?.getAttribute("rel")).toBe("noopener noreferrer");
  });
});

describe("renderMarkdown: распорка под время", () => {
  it("в конце последней строки текста есть распорка", () => {
    const host = render("Привет\n\nЕщё абзац", true);
    expect(host.querySelectorAll(".message__spacer")).toHaveLength(1);
    expect(host.querySelector("p:last-child .message__spacer")).not.toBeNull();
  });

  it("у списка распорка в последнем пункте", () => {
    const host = render("- раз\n- два", true);
    expect(host.querySelector("li:last-child .message__spacer")).not.toBeNull();
  });

  it("у заголовка распорка внутри заголовка", () => {
    expect(render("### Планы", true).querySelector("h3 .message__spacer")).not.toBeNull();
  });

  it("если сообщение кончается блоком кода, распорки нет: время под блоком", () => {
    expect(render("Вот код:\n\n```\nfn x() {}\n```", true).querySelector(".message__spacer")).toBeNull();
  });

  it("без timeSpacer (состояние «не отправилось») распорки нет", () => {
    expect(render("Купить билеты", false).querySelector(".message__spacer")).toBeNull();
  });
});
