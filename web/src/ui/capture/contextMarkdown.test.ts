import { describe, expect, it } from "vitest";
import { renderContextMarkdown } from "./contextMarkdown";

const labels = { done: "Готово", todo: "Не готово" };
const render = (source: string) => {
  const host = document.createElement("div");
  host.innerHTML = renderContextMarkdown(source, labels);
  return host;
};

describe("renderContextMarkdown", () => {
  it("ссылка остаётся текстом: ни <a>, ни адреса, ни фокуса", () => {
    const host = render("Читал [статью](https://example.com/a) и https://example.org/b тоже");
    expect(host.querySelector("a")).toBeNull();
    expect(host.innerHTML).not.toContain("href");
    expect(host.querySelectorAll(".capture-link")).toHaveLength(2);
    expect(host.textContent).toContain("Читал статью и https://example.org/b тоже");
  });

  it("блок кода не берёт фокус, но остаётся блоком", () => {
    const host = render("```\nfn main() {}\n```");
    expect(host.querySelector("pre")).not.toBeNull();
    expect(host.querySelector("[tabindex]")).toBeNull();
    expect(host.querySelector("pre")?.textContent).toContain("fn main() {}");
  });

  it("остальной Markdown свёрстан как в ленте: заголовок, жирный, список задач", () => {
    const host = render("### План\n**важно**\n\n- [x] сделано\n- [ ] ждёт");
    expect(host.querySelector("h3")?.textContent).toBe("План");
    expect(host.querySelector("strong")?.textContent).toBe("важно");
    expect(host.querySelectorAll("li.task")).toHaveLength(2);
    expect(host.querySelector(".task--done .task__box")).toHaveAttribute("aria-label", "Готово");
  });

  it("распорки под время нет: время в запись контекста вставляется отдельной колонкой", () => {
    expect(render("текст").querySelector(".message__spacer")).toBeNull();
  });

  it("сырой HTML по-прежнему не рендерится", () => {
    const host = render('<img src=x onerror="alert(1)"> <script>alert(1)</script>');
    expect(host.querySelector("img, script")).toBeNull();
  });
});
