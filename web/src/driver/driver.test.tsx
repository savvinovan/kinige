// The screens driver (KINIG-92, scripts/screens/driver.ts): name matching, the steps, and the channel to the Rust side.
import { act, cleanup, render } from "@testing-library/react";
import { useState } from "react";
import { startDriver as startDriverInNormalBuild } from "virtual:kinige-driver";
import { afterEach, describe, expect, it, vi } from "vitest";
import { findAll, roleOf } from "../../../scripts/screens/driver-dom";
import { DRIVER_MARKER, runStep, startDriver, type DriverHost } from "../../../scripts/screens/driver";

const FAST = { patienceMs: 40 };

function page(html: string): void {
  document.body.innerHTML = html;
}

afterEach(() => {
  cleanup();
  document.body.innerHTML = "";
});

describe("name matching", () => {
  it("finds an element by aria-label, aria-labelledby, form label, title and text", () => {
    page(`
      <button id="a" aria-label="Alpha"></button>
      <span id="b-name">Bravo</span><button id="b" aria-labelledby="b-name"></button>
      <label for="c">Charlie</label><input id="c" />
      <button id="d" title="Delta"></button>
      <button id="e">  Echo &nbsp; now </button>`);
    const idOf = (name: string) => findAll(document, { name })[0]?.id;
    expect(idOf("Alpha")).toBe("a");
    expect(idOf("Bravo")).toBe("b");
    expect(findAll(document, { name: "Bravo", role: "button" })[0]?.id).toBe("b");
    expect(idOf("Charlie")).toBe("c");
    expect(idOf("Delta")).toBe("d");
    expect(idOf("Echo now")).toBe("e");
  });

  it("prefers an exact name to one that only contains the words, and an operable element to plain text", () => {
    page(`<span>Play</span><button id="speed" aria-label="Playback speed 1x"></button><button id="play" aria-label="Play"></button>`);
    const names = findAll(document, { name: "Play" }).map((element) => element.id || element.tagName);
    expect(names).toEqual(["play", "SPAN", "speed"]);
  });

  it("narrows by role and picks the nth match", () => {
    page(`<button id="1">Save</button><a id="2" href="#x">Save</a><button id="3">Save</button>`);
    expect(findAll(document, { name: "Save", role: "link" }).map((e) => e.id)).toEqual(["2"]);
    expect(findAll(document, { name: "Save", role: "button" }).map((e) => e.id)).toEqual(["1", "3"]);
    expect(roleOf(document.getElementById("2") as Element)).toBe("link");
  });

  it("skips hidden elements", () => {
    page(`
      <button hidden>Ghost</button>
      <div aria-hidden="true"><button>Ghost</button></div>
      <div style="display: none"><button>Ghost</button></div>
      <button style="visibility: hidden">Ghost</button>
      <button id="seen">Ghost</button>`);
    expect(findAll(document, { name: "Ghost" }).map((e) => e.id)).toEqual(["seen"]);
  });

  it("does not let a container answer to the words of everything inside it", () => {
    page(`<div id="box"><p id="inner">Hello there</p></div>`);
    expect(findAll(document, { name: "Hello" }).map((e) => e.id)).toEqual(["inner"]);
  });
});

describe("press, menu, scroll", () => {
  it("presses with the pointer sequence and the click", async () => {
    page(`<button aria-label="Go">x</button>`);
    const seen: string[] = [];
    const button = document.querySelector("button") as HTMLElement;
    for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]) button.addEventListener(type, () => seen.push(type));
    expect(await runStep({ kind: "press", name: "Go" }, FAST)).toEqual({ ok: true });
    expect(seen).toEqual(["pointerdown", "mousedown", "pointerup", "mouseup", "click"]);
    expect(document.activeElement).toBe(button);
  });

  it("says what was not found", async () => {
    page(`<button>Other</button>`);
    expect(await runStep({ kind: "press", name: "Missing", role: "button" }, FAST)).toEqual({ ok: false, error: 'not found: "Missing" (button)' });
  });

  it("waits a moment for an element that is about to show", async () => {
    page("");
    const late = vi.fn();
    setTimeout(() => {
      page(`<button id="late">Late</button>`);
      document.getElementById("late")?.addEventListener("click", late);
    }, 60);
    expect(await runStep({ kind: "press", name: "Late" }, { patienceMs: 600 })).toEqual({ ok: true });
    expect(late).toHaveBeenCalledTimes(1);
  });

  it("does not press a disabled button", async () => {
    page(`<button disabled aria-label="Off"></button>`);
    const answer = await runStep({ kind: "press", name: "Off" }, FAST);
    expect(answer).toEqual({ ok: false, error: "disabled: Off" });
  });

  it("sends contextmenu with a right button for menu", async () => {
    page(`<div role="button" aria-haspopup="menu" aria-label="Chat">Chat</div>`);
    const received = vi.fn();
    document.querySelector("div")?.addEventListener("contextmenu", (event) => received((event as MouseEvent).button));
    expect(await runStep({ kind: "menu", name: "Chat" }, FAST)).toEqual({ ok: true });
    expect(received).toHaveBeenCalledWith(2);
  });

  it("scrolls into view", async () => {
    page(`<p>Row</p>`);
    const scroll = vi.fn();
    (document.querySelector("p") as HTMLElement).scrollIntoView = scroll;
    expect(await runStep({ kind: "scroll", name: "Row" }, FAST)).toEqual({ ok: true });
    expect(scroll).toHaveBeenCalled();
    expect((await runStep({ kind: "scroll", name: "Nothing" }, FAST)).ok).toBe(false);
  });
});

describe("type and key", () => {
  function Field() {
    const [value, setValue] = useState("");
    return (
      <>
        <input aria-label="Search" value={value} onChange={(event) => setValue(event.target.value)} />
        <output data-testid="state">{value}</output>
      </>
    );
  }

  it("types into a React-controlled input so that its state follows", async () => {
    const view = render(<Field />);
    expect(await runStep({ kind: "type", name: "Search", text: "voice" }, FAST)).toEqual({ ok: true });
    expect(view.getByTestId("state").textContent).toBe("voice");
  });

  it("types into the focused textarea when no name is given", async () => {
    page(`<textarea id="t"></textarea>`);
    (document.getElementById("t") as HTMLElement).focus();
    expect(await runStep({ kind: "type", text: "hello" }, FAST)).toEqual({ ok: true });
    expect((document.getElementById("t") as HTMLTextAreaElement).value).toBe("hello");
  });

  it("refuses to type where there is no text field", async () => {
    page(`<button aria-label="Search">x</button>`);
    expect((await runStep({ kind: "type", name: "Search", text: "x" }, FAST)).ok).toBe(false);
    (document.activeElement as HTMLElement | null)?.blur();
    expect(await runStep({ kind: "type", text: "x" }, FAST)).toEqual({ ok: false, error: "no text field to type into" });
  });

  it("sends keydown and keyup with the modifiers to the focused element", async () => {
    page(`<input id="f" />`);
    const field = document.getElementById("f") as HTMLElement;
    field.focus();
    const seen: string[] = [];
    for (const type of ["keydown", "keyup"]) field.addEventListener(type, (event) => seen.push(`${type}:${(event as KeyboardEvent).key}:${(event as KeyboardEvent).metaKey}`));
    expect(await runStep({ kind: "key", key: "k", code: "KeyK", mods: ["meta"] }, FAST)).toEqual({ ok: true });
    expect(seen).toEqual(["keydown:k:true", "keyup:k:true"]);
    expect((await runStep({ kind: "key", key: "" }, FAST)).ok).toBe(false);
  });
});

describe("wait, has, text", () => {
  it("has answers yes or no at once", async () => {
    page(`<h1>Inbox</h1>`);
    expect(await runStep({ kind: "has", name: "Inbox" }, FAST)).toEqual({ ok: true, value: "yes" });
    expect(await runStep({ kind: "has", name: "Archive" }, FAST)).toEqual({ ok: true, value: "no" });
  });

  it("waits for an element, and for it to go", async () => {
    page("");
    setTimeout(() => page(`<p>Ready</p>`), 20);
    expect(await runStep({ kind: "wait", name: "Ready", timeoutMs: 500 })).toEqual({ ok: true });
    setTimeout(() => page(""), 20);
    expect(await runStep({ kind: "wait", name: "Ready", gone: true, timeoutMs: 500 })).toEqual({ ok: true });
  });

  it("reports a timeout with the name and the time", async () => {
    page("");
    expect(await runStep({ kind: "wait", name: "Never", timeoutMs: 60 })).toEqual({ ok: false, error: 'not found after 60 ms: "Never"' });
    page(`<p>Stuck</p>`);
    expect(await runStep({ kind: "wait", name: "Stuck", gone: true, timeoutMs: 60 })).toEqual({ ok: false, error: 'still there after 60 ms: "Stuck"' });
  });

  it("text holds the page text, every label and title, and typed values", async () => {
    page(`<p>Visible words</p><button aria-label="Labelled"></button><span title="Tip">x</span><input value="typed" />`);
    const answer = await runStep({ kind: "text" });
    expect(answer.ok && answer.value).toEqual(expect.stringContaining("Visible words"));
    for (const word of ["Labelled", "Tip", "typed"]) expect(answer.ok && answer.value).toEqual(expect.stringContaining(word));
  });
});

describe("garbage", () => {
  it("answers an error to anything that is not a step, and never throws", async () => {
    page("<p>x</p>");
    for (const garbage of [null, undefined, 7, "press", {}, { kind: "eval", code: "1" }, { kind: "front" }, { kind: "press" }, { kind: "press", name: 5 }, { kind: "type" }, { kind: "wait", name: "x" }]) {
      const answer = await runStep(garbage, FAST);
      expect(answer.ok).toBe(false);
    }
  });
});

describe("the channel", () => {
  function fakeHost() {
    const handlers: ((event: { payload: unknown }) => void)[] = [];
    const invoke = vi.fn(async () => undefined);
    const host: DriverHost = {
      listen: vi.fn(async (_event, handler) => {
        handlers.push(handler);
      }),
      invoke,
    };
    return { host, handlers, invoke };
  }

  it("says it is ready after it listens, and answers a step by id", async () => {
    page(`<button aria-label="Go">x</button>`);
    const { host, handlers, invoke } = fakeHost();
    startDriver(host);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("screens_driver_reply", { message: { kind: "ready" } }));
    expect(host.listen).toHaveBeenCalledWith("screens-driver-step", expect.any(Function));
    await act(async () => {
      handlers[0]?.({ payload: { id: "r1", step: { kind: "has", name: "Go" } } });
    });
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("screens_driver_reply", { message: { kind: "reply", id: "r1", ok: true, value: "yes", error: undefined } }),
    );
    handlers[0]?.({ payload: { id: "r2", step: { kind: "nonsense" } } });
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("screens_driver_reply", { message: { kind: "reply", id: "r2", ok: false, value: undefined, error: "unknown step: nonsense" } }),
    );
  });

  it("ignores a payload without an id", async () => {
    const { host, handlers, invoke } = fakeHost();
    startDriver(host);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    handlers[0]?.({ payload: { step: { kind: "text" } } });
    handlers[0]?.({ payload: null });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("leaves the marker on the document, and the normal build has no driver at all", () => {
    expect(document.documentElement.dataset.screensDriver).toBe(DRIVER_MARKER);
    expect(startDriverInNormalBuild).toBeUndefined();
  });
});
