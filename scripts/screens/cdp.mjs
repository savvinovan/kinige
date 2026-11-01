// Drives the WebView2 pages of Kinige through the DevTools protocol (Playwright connectOverCDP) for the Windows screens
// scenario (scripts/screens/10a.ps1). One call = one action; the scenario calls it as a command:
//   node scripts/screens/cdp.mjs <port> <main|settings> <action> [arguments]
//   ready                    wait until the page has content
//   text                     print the visible text and the accessible names (JSON) for the personal-path check
//   has <text>               exit 0 if the text is on the page, 1 if not (after a short wait)
//   click <name>             press the button, menu item, radio, tab or link with that accessible name
//   fill <label> <text>      type into the text field with that accessible name
// The debugging port exists only in the CI scenario: the scenario sets WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS for the app it
// starts, and Chromium binds the port to 127.0.0.1 only. The app itself has no code for this.
import { createRequire } from "node:module";

const WAIT_MS = 20_000;
const ROLES = ["button", "menuitem", "radio", "checkbox", "tab", "link", "treeitem", "option"];

const [port, which, action, ...rest] = process.argv.slice(2);
const require = createRequire(new URL("../../web/package.json", import.meta.url));
const { chromium } = require("@playwright/test");

const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
try {
  const page = await findPage();
  switch (action) {
    case "ready":
      await page.waitForFunction(() => document.body && document.body.innerText.trim().length > 20, null, { timeout: WAIT_MS });
      break;
    case "text":
      console.log(JSON.stringify(await readText(page)));
      break;
    case "has":
      process.exitCode = (await waitFor(async () => (await readText(page)).all.includes(rest[0]))) ? 0 : 1;
      break;
    case "click":
      await click(page, rest[0]);
      break;
    case "fill":
      await fill(page, rest[0], rest[1]);
      break;
    default:
      throw new Error(`unknown action ${action}`);
  }
} catch (error) {
  console.error(`cdp ${which} ${action}: ${String(error.message).split("\n")[0]}`);
  process.exitCode = 2;
} finally {
  // For a connection made over CDP this only disconnects: the app keeps running.
  await browser.close();
}

// The page of the window: the settings window has `#settings` in its address, the capture window `#capture`.
async function findPage() {
  const pages = await waitFor(async () => {
    const all = browser.contexts().flatMap((context) => context.pages());
    const wanted = all.filter((page) => {
      const hash = new URL(page.url()).hash;
      return which === "settings" ? hash === "#settings" : hash !== "#settings" && hash !== "#capture" && page.url() !== "about:blank";
    });
    return wanted.length > 0 ? wanted : null;
  });
  if (!pages) throw new Error(`no ${which} page`);
  return pages[0];
}

// Visible text plus everything a screen reader would read: names, titles, alternative texts.
async function readText(page) {
  const found = await page.evaluate(() => {
    const names = [];
    for (const el of document.querySelectorAll("[aria-label],[title],[alt],[placeholder]")) {
      for (const attribute of ["aria-label", "title", "alt", "placeholder"]) {
        const value = el.getAttribute(attribute);
        if (value) names.push(value);
      }
    }
    return { text: document.body ? document.body.innerText : "", names, url: location.href, title: document.title };
  });
  return { ...found, all: [found.text, ...found.names].join("\n") };
}

async function waitFor(check) {
  const deadline = Date.now() + WAIT_MS;
  for (;;) {
    const value = await check();
    if (value) return value;
    if (Date.now() > deadline) return null;
    await new Promise((resolve) => setTimeout(resolve, 300));
  }
}

// Exact accessible name first, then a substring (chat buttons read "Name , entries: N").
async function click(page, name) {
  const target = await waitFor(async () => {
    for (const exact of [true, false]) {
      for (const role of ROLES) {
        const locator = page.getByRole(role, { name, exact });
        if ((await locator.count()) > 0) return locator.first();
      }
    }
    const byText = page.getByText(name, { exact: false });
    return (await byText.count()) > 0 ? byText.first() : null;
  });
  if (!target) throw new Error(`nothing to press for "${name}"`);
  await target.click({ timeout: WAIT_MS });
}

async function fill(page, label, text) {
  const field = page.getByRole("textbox", { name: label, exact: false }).first();
  await field.waitFor({ timeout: WAIT_MS });
  await field.fill(text);
}
