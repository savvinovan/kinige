import { describe, expect, it } from "vitest";
import { fixture, spec, type Spec } from "./apiContractFixtures";
import { INBOX_ID, errorCode } from "./KinigeApi";
import { MockApi, mockAttachment } from "./MockApi";
import { applyDetails, normalizeDescription, normalizeName } from "./mockDetails";
import { mockMessage } from "./mockData";

// Slice 7b: the counting of the details of an attachment, pinned by `fixtures/api-contract/attachment-text.json` (the same file
// `crates/kinige-store/tests/api_contract/` runs against `normalize_name` and `normalize_description`), and the commands of `MockApi`.

interface Case {
  name: string;
  expect?: Spec | null;
  error?: boolean;
}
interface NameCase extends Case {
  stem: Spec;
  original: Spec;
}
interface DescriptionCase extends Case {
  text: Spec;
}
interface Details {
  names: NameCase[];
  descriptions: DescriptionCase[];
}

/** The result, `null` for none, or `{ error: code }`. */
function outcome(run: () => string | null): string | null | { error: string } {
  try {
    return run();
  } catch (error) {
    return { error: errorCode(error) };
  }
}

const wanted = (entry: Case): string | null | { error: string } =>
  entry.error ? { error: "attachment_details_invalid" } : entry.expect === null || entry.expect === undefined ? null : spec(entry.expect);

describe("fixtures/api-contract/attachment-text", () => {
  it("names: the typed stem and the fixed original extension", () => {
    for (const entry of fixture<Details>("attachment-text").names) {
      const got = outcome(() => normalizeName(spec(entry.stem), spec(entry.original)));
      expect(got, entry.name).toEqual(wanted(entry));
    }
  });

  it("descriptions: normalized, then counted in scalar values", () => {
    for (const entry of fixture<Details>("attachment-text").descriptions) {
      expect(outcome(() => normalizeDescription(spec(entry.text))), entry.name).toEqual(wanted(entry));
    }
  });

  it("a lone surrogate is refused (it cannot reach Rust, so the file does not hold it)", () => {
    expect(outcome(() => normalizeName("a\ud800b", "x.jpg"))).toEqual({ error: "attachment_details_invalid" });
    expect(outcome(() => normalizeDescription("a\udc00b"))).toEqual({ error: "attachment_details_invalid" });
    // A pair is one character, not a lone surrogate.
    expect(normalizeDescription("a😀b")).toBe("a\u{1f600}b");
  });
});

describe("MockApi: the details commands", () => {
  const photo = mockAttachment("p1", "IMG_0001.jpg");
  const pdf = mockAttachment("f1", "menu.pdf");

  it("applyDetails sends only what changed, clears with null and refuses a spoiler on a file card", () => {
    const renamed = applyDetails(photo, { name: "beach", description: "Beach, July", spoiler: true });
    expect([renamed.name, renamed.filename, renamed.description, renamed.spoiler]).toEqual(["beach.jpg", "IMG_0001.jpg", "Beach, July", true]);
    const same = applyDetails(renamed, {});
    expect(same).toEqual(renamed);
    const cleared = applyDetails(renamed, { name: null, description: null });
    expect([cleared.name, cleared.description, cleared.spoiler]).toEqual(["IMG_0001.jpg", null, true]);
    for (const garbage of [{ spoiler: true }, { spoiler: false }]) {
      expect(() => applyDetails(pdf, garbage)).toThrow();
    }
    for (const garbage of [{ spoiler: null }, { name: 7 }, { description: [] }, { filename: "x.jpg" }, { blobKey: "x" }]) {
      expect(() => applyDetails(photo, garbage as never), JSON.stringify(garbage)).toThrow();
    }
  });

  it("a draft tile keeps its details until it is sent, and a sent attachment is edited in place", async () => {
    const api = new MockApi({ messages: [mockMessage({ id: "m1", chatId: INBOX_ID, bodyMd: "x", attachments: [photo, pdf] })] });
    const tile = await api.attachPaste(INBOX_ID, new Uint8Array(4), "scan.png");
    const edited = await api.setDraftAttachmentDetails(tile.id, { name: "map", spoiler: true });
    expect([edited.name, edited.spoiler]).toEqual(["map.png", true]);
    expect((await api.getDraft(INBOX_ID)).attachments).toEqual([edited]);
    const sent = await api.sendMessage(INBOX_ID, "", null, [tile.id]);
    expect(sent.attachments).toEqual([edited]);

    const shown = await api.setAttachmentDetails("p1", { description: "Beach" });
    expect(shown.description).toBe("Beach");
    const feed = await api.listMessages(INBOX_ID);
    expect(feed.messages.find((m) => m.id === "m1")?.attachments[0]).toEqual(shown);
  });

  it("errors: an unknown id, a rule broken, an entry in the trash", async () => {
    const api = new MockApi({ messages: [mockMessage({ id: "m1", chatId: INBOX_ID, bodyMd: "x", attachments: [photo] })] });
    const code = (run: Promise<unknown>) => run.then(() => "ok", errorCode);
    expect(await code(api.setAttachmentDetails("ghost", { name: "x" }))).toBe("attachment_not_found");
    expect(await code(api.setDraftAttachmentDetails("ghost", { name: "x" }))).toBe("attachment_not_found");
    expect(await code(api.setAttachmentDetails("p1", { name: "a/b" }))).toBe("attachment_details_invalid");
    expect(await code(api.setAttachmentDetails("p1", { description: "x".repeat(1025) }))).toBe("attachment_details_invalid");
    expect((await api.listMessages(INBOX_ID)).messages[0]?.attachments[0]?.name).toBe("IMG_0001.jpg");
    await api.deleteMessage("m1");
    expect(await code(api.setAttachmentDetails("p1", { name: "x" }))).toBe("message_in_trash");
  });
});
