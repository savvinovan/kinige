import { describe, expect, it } from "vitest";
import { renderMarkdown, sanitizeHtml } from "./markdown";

const render = (source: string, timeSpacer = false) => {
  const host = document.createElement("div");
  host.innerHTML = renderMarkdown(source, { timeSpacer, taskLabels: { done: "Выполнено", todo: "Не выполнено" } });
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

  it("блок кода: текст сохраняется как есть, прокрутки нет (в порядок Tab не входит), внутри нет разметки", () => {
    const host = render("```rust\nfn main() {\n    println!(\"<b>\");\n}\n```");
    const pre = host.querySelector("pre");
    expect(pre?.hasAttribute("tabindex")).toBe(false);
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

describe("renderMarkdown: теги (слайс 3)", () => {
  const withTags = (source: string, tags: string[]) => {
    const host = document.createElement("div");
    host.innerHTML = renderMarkdown(source, { timeSpacer: false, taskLabels: { done: "Выполнено", todo: "Не выполнено" }, tags });
    return host;
  };
  const found = (host: HTMLElement) => [...host.querySelectorAll(".tag")].map((tag) => tag.textContent);

  it("подсвечивается только тег из списка Rust; регистр не важен, текст остаётся как написан", () => {
    const host = withTags("#Идея и #другое, потом #идея", ["идея"]);
    expect(found(host)).toEqual(["#Идея", "#идея"]);
    expect(host.querySelector(".tag")?.getAttribute("role")).toBe("button");
    expect(host.querySelector(".tag")?.getAttribute("tabindex")).toBe("0");
    expect(host.textContent?.trim()).toBe("#Идея и #другое, потом #идея");
  });

  it("без списка тегов или с пустым списком ничего не подсвечивается", () => {
    expect(found(render("#идея"))).toEqual([]);
    expect(found(withTags("#идея", []))).toEqual([]);
  });

  it("слово — буквы любого алфавита, цифры и подчёркивание", () => {
    const host = withTags("#день_1 #tag2 #日本語", ["день_1", "tag2", "日本語"]);
    expect(found(host)).toEqual(["#день_1", "#tag2", "#日本語"]);
  });

  it("тег в начале строки и после знака препинания", () => {
    const host = withTags("(#идея) «#идея»,\n#идея", ["идея"]);
    expect(found(host)).toHaveLength(3);
  });

  it("не тег: после буквы, цифры, подчёркивания, #, & и обратной косой", () => {
    expect(found(withTags("a#идея 1#идея _#идея ##идея &#идея", ["идея"]))).toEqual([]);
    expect(found(withTags("\\#идея", ["идея"]))).toEqual([]);
  });

  it("не тег: внутри кода, блока кода и ссылки", () => {
    const source = "`#идея`\n\n```\n#идея\n```\n\n[#идея](https://example.com) https://example.com/#идея";
    expect(found(withTags(source, ["идея"]))).toEqual([]);
  });

  it("заголовок «# слово» — заголовок, а не тег; «#слово» в заголовке — тег", () => {
    const host = withTags("# Идея\n\n## про #идея", ["идея"]);
    expect(host.querySelector("h1")?.textContent).toBe("Идея");
    expect(found(host)).toEqual(["#идея"]);
  });

  it("тег внутри выделения, списка и задачи; распорка под время по-прежнему в конце", () => {
    const host = document.createElement("div");
    host.innerHTML = renderMarkdown("**важно #идея**\n\n- [ ] сделать #идея", {
      timeSpacer: true,
      taskLabels: { done: "Выполнено", todo: "Не выполнено" },
      tags: ["идея"],
    });
    expect(found(host)).toEqual(["#идея", "#идея"]);
    expect(host.querySelector("strong .tag")).not.toBeNull();
    expect(host.querySelector("li:last-child .message__spacer")).not.toBeNull();
  });

  it("опасное в слове тега не попадает в разметку", () => {
    const host = withTags("#идея<script>", ["идея"]);
    expect(host.querySelector("script")).toBeNull();
    expect(host.textContent?.trim()).toBe("#идея<script>");
  });
});

describe("renderMarkdown: кликабельные квадраты задач (слайс 3)", () => {
  const interactive = (source: string) => {
    const host = document.createElement("div");
    host.innerHTML = renderMarkdown(source, {
      timeSpacer: false,
      taskLabels: { done: "Выполнено", todo: "Не выполнено", item: "Пункт выполнен" },
      interactiveTasks: true,
    });
    return host;
  };

  it("квадрат — флажок: role=checkbox, aria-checked, tabindex 0 и имя без состояния", () => {
    const boxes = [...interactive("- [x] раз\n- [ ] два").querySelectorAll(".task__box")];
    expect(boxes.map((box) => box.getAttribute("role"))).toEqual(["checkbox", "checkbox"]);
    expect(boxes.map((box) => box.getAttribute("aria-checked"))).toEqual(["true", "false"]);
    expect(boxes.map((box) => box.getAttribute("tabindex"))).toEqual(["0", "0"]);
    expect(boxes.map((box) => box.getAttribute("aria-label"))).toEqual(["Пункт выполнен", "Пункт выполнен"]);
  });

  it("без interactiveTasks квадраты остаются картинками, aria-checked нет", () => {
    expect(render("- [x] раз").querySelector(".task__box")?.getAttribute("aria-checked")).toBeNull();
  });

  it("DOMPurify оставляет aria-checked только на разрешённых тегах и не пропускает прочие aria-атрибуты", () => {
    const host = document.createElement("div");
    host.innerHTML = sanitizeHtml('<span class="task__box" role="checkbox" aria-checked="true" aria-live="assertive" onclick="x()"></span>');
    const box = host.querySelector("span");
    expect(box?.getAttribute("aria-checked")).toBe("true");
    expect(box?.hasAttribute("aria-live")).toBe(false);
    expect(box?.hasAttribute("onclick")).toBe(false);
  });
});

describe("renderMarkdown: найденные слова (поиск, слайс 4)", () => {
  const marked = (source: string, marks: string[]) => {
    const host = document.createElement("div");
    host.innerHTML = renderMarkdown(source, {
      timeSpacer: false,
      taskLabels: { done: "Выполнено", todo: "Не выполнено" },
      tags: ["книга"],
      marks,
    });
    return host;
  };

  it("целые слова с тем же регистром оборачиваются в mark; части слов и чужой регистр — нет", () => {
    const host = marked("Книга про книгу, книгами и Книга", ["книгу", "Книга"]);
    expect([...host.querySelectorAll("mark")].map((m) => m.textContent)).toEqual(["Книга", "книгу", "Книга"]);
    expect(host.textContent?.trim()).toBe("Книга про книгу, книгами и Книга");
  });

  it("внутри ссылок, кода и тегов подсветки нет; слова в жирном выделяются", () => {
    const host = marked("[книга](https://example.org/k) `книга` #книга **книга**", ["книга"]);
    expect([...host.querySelectorAll("mark")].map((m) => m.parentElement?.tagName)).toEqual(["STRONG"]);
  });

  it("html слов экранируется: спецзнаки в тексте остаются текстом", () => {
    const host = marked("a<b> слово & слово", ["слово"]);
    expect(host.querySelectorAll("mark")).toHaveLength(2);
    expect(host.textContent?.trim()).toBe("a<b> слово & слово");
    expect(host.querySelector("b")).toBeNull();
  });

  it("санитайзер пропускает mark, но не его атрибуты", () => {
    expect(sanitizeHtml('<mark onclick="x()" class="y">слово</mark>')).toBe('<mark class="y">слово</mark>');
  });
});
