import { describe, expect, it } from "vitest";
import { mergeLatestPage } from "./mergeFeed";

const m = (id: string, createdAt: number, journalDate = "2026-10-21", body = id) => ({ id, journalDate, createdAt, body });

describe("mergeLatestPage", () => {
  it("страница — вся история: лента равна странице", () => {
    const loaded = [m("a", 1), m("b", 2)];
    expect(mergeLatestPage(loaded, [m("b", 2), m("c", 3)], false).map((x) => x.id)).toEqual(["b", "c"]);
  });

  it("есть более старые: подгруженное старше страницы остаётся, покрытое страницей берётся из неё", () => {
    const loaded = [m("a", 1), m("b", 2), m("c", 3, "2026-10-21", "старый текст")];
    const page = [m("c", 3, "2026-10-21", "дописано"), m("d", 4)];
    const merged = mergeLatestPage(loaded, page, true);
    expect(merged.map((x) => x.id)).toEqual(["a", "b", "c", "d"]);
    expect(merged[2]?.body).toBe("дописано");
  });

  it("запись, исчезнувшая внутри окна страницы, исчезает и из ленты", () => {
    const loaded = [m("a", 1), m("b", 2), m("c", 3)];
    expect(mergeLatestPage(loaded, [m("a", 1), m("c", 3)], false).map((x) => x.id)).toEqual(["a", "c"]);
    expect(mergeLatestPage(loaded, [m("b", 2), m("c", 3)], true).map((x) => x.id)).toEqual(["a", "b", "c"]);
  });

  it("порядок учитывает день журнала и id при равном времени", () => {
    const loaded = [m("x", 5, "2026-10-20"), m("a", 9, "2026-10-21")];
    const page = [m("b", 9, "2026-10-21"), m("c", 9, "2026-10-21")];
    expect(mergeLatestPage(loaded, page, true).map((x) => x.id)).toEqual(["x", "a", "b", "c"]);
  });

  it("пустая страница: лента равна пустой странице", () => {
    expect(mergeLatestPage([m("a", 1)], [], true)).toEqual([]);
  });
});
