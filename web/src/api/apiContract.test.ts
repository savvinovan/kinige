import { describe, expect, it } from "vitest";
import { fixture, spec, type Spec } from "./apiContractFixtures";
import { INBOX_ID, errorCode } from "./KinigeApi";
import { mockAttachment } from "./MockApi";
import { resolveColor } from "./mockColor";
import { mockStage } from "./mockForest";
import {
  LIMITS,
  attachmentKind,
  canAppendTo,
  dayChat,
  daysLeft,
  isRealDate,
  joinBody,
  mockJournalDate,
  normalizeIcon,
  normalizeTitle,
  replyQuote,
  yearsLater,
} from "./mockRules";
import { mockTags } from "./mockTags";
import { toggleTaskItem } from "./mockTasks";
import { plainSnippet } from "./mockText";
import type { ChatColor } from "./generated/ChatColor";
import type { ReplyTo } from "./generated/ReplyTo";

// The rules `MockApi` repeats from Rust, run on the files of `fixtures/api-contract/`. The same files are run against the real
// functions by `crates/kinige-store/tests/api_contract/`: if a case fails here and passes there, the mock is wrong.

interface Cases<T> {
  cases: T[];
}

/** The result of a function that may throw an API error: the value, or `{ error: code }`. */
function outcome<T>(run: () => T): T | { error: string } {
  try {
    return run();
  } catch (error) {
    return { error: errorCode(error) };
  }
}

/** What a case of `chat-input.json` expects: the text (repeated if the case says so) or the error. */
function expectation(entry: { expected?: string; error?: string; repeat?: number }): string | { error: string } {
  if (entry.error) return { error: entry.error };
  return (entry.expected ?? "").repeat(entry.repeat ?? 1);
}

describe("fixtures/api-contract: the pure rules of the mock", () => {
  it("chat-color: Inbox is grey, a pick wins, otherwise FNV-1a over the UTF-8 bytes", () => {
    for (const entry of fixture<Cases<{ id: string; picked: ChatColor | null; expected: string }>>("chat-color").cases) {
      expect(resolveColor(entry.id, entry.picked), JSON.stringify(entry)).toBe(entry.expected);
    }
  });

  it("date-valid: a real day from the year 1000", () => {
    for (const entry of fixture<Cases<{ text: string; valid: boolean }>>("date-valid").cases) {
      expect(isRealDate(entry.text), JSON.stringify(entry)).toBe(entry.valid);
    }
  });

  it("journal-day-utc: the 04:00 boundary", () => {
    for (const entry of fixture<Cases<{ atUtc: string; day: string }>>("journal-day-utc").cases) {
      expect(mockJournalDate(new Date(entry.atUtc)), entry.atUtc).toBe(entry.day);
    }
  });

  it("chat-input: the name and the emoji of a chat", () => {
    type Entry = { text: string | null; expected?: string; error?: string; repeat?: number };
    const file = fixture<{ titles: Entry[]; icons: Entry[] }>("chat-input");
    for (const entry of file.titles) {
      const text = (entry.text ?? "").repeat(entry.repeat ?? 1);
      expect(outcome(() => normalizeTitle(text)), JSON.stringify(entry)).toEqual(expectation(entry));
    }
    for (const entry of file.icons) {
      const text = entry.text === null ? null : entry.text.repeat(entry.repeat ?? 1);
      expect(outcome(() => normalizeIcon(text)), JSON.stringify(entry)).toEqual(expectation(entry));
    }
  });

  it("append-target: the entry capture appends to", () => {
    type Entry = { now: string; today: string; origin: string; ageMs: number; journalDate: string; expected: boolean };
    for (const entry of fixture<Cases<Entry>>("append-target").cases) {
      const now = Date.parse(entry.now);
      expect(mockJournalDate(new Date(now)), JSON.stringify(entry)).toBe(entry.today);
      const candidate = { origin: entry.origin, createdAt: now - entry.ageMs, journalDate: entry.journalDate };
      expect(canAppendTo(candidate, now), JSON.stringify(entry)).toBe(entry.expected);
    }
  });

  it("append-join: the text after appending", () => {
    for (const entry of fixture<Cases<{ old: string; addition: string; expected: string }>>("append-join").cases) {
      expect(joinBody(entry.old, entry.addition), JSON.stringify(entry)).toBe(entry.expected);
    }
  });

  it("reply-quote: years later, the excerpt and the quote states", () => {
    type Quote = {
      id: string;
      replyDate: string;
      target: { chatId: string; journalDate: string; bodyMd: string; inTrash: boolean } | null;
      expected: Omit<ReplyTo, "id" | "attachments">;
    };
    const file = fixture<{
      yearsLater: { old: string; reply: string; expected: number }[];
      excerpts: { body: Spec; expected: Spec }[];
      quotes: Quote[];
    }>("reply-quote");
    for (const entry of file.yearsLater) {
      expect(yearsLater(entry.old, entry.reply), JSON.stringify(entry)).toBe(entry.expected);
    }
    for (const entry of file.excerpts) {
      expect(plainSnippet(spec(entry.body), LIMITS.replyExcerptChars), JSON.stringify(entry).slice(0, 80)).toBe(spec(entry.expected));
    }
    for (const entry of file.quotes) {
      const target = entry.target && { ...entry.target, attachments: { photos: 0, files: 0, voices: 0, voiceMs: null, name: null } };
      const { id, attachments, ...quote } = replyQuote(entry.id, entry.replyDate, target ?? undefined);
      expect(id).toBe(entry.id);
      expect(attachments.photos).toBe(0);
      expect(quote, entry.id).toEqual(entry.expected);
    }
  });

  it("trash-days: whole days left", () => {
    const deletedAt = 1_000_000_000_000;
    for (const entry of fixture<Cases<{ sinceDeletedMs: number; expected: number }>>("trash-days").cases) {
      expect(daysLeft(deletedAt, deletedAt + entry.sinceDeletedMs), JSON.stringify(entry)).toBe(entry.expected);
    }
  });

  it("constants: sizes and limits, and every limit of the mock is pinned", () => {
    const cases = fixture<Cases<{ name: keyof typeof LIMITS; value: number }>>("constants").cases;
    for (const entry of cases) expect(LIMITS[entry.name], entry.name).toBe(entry.value);
    // `recentEmoji` is pinned by the scenario `recent-emoji.json`: Rust keeps that constant private.
    const pinned = [...cases.map((entry) => entry.name), "recentEmoji"].sort();
    expect(Object.keys(LIMITS).sort()).toEqual(pinned);
  });

  it("tags: the tags of an entry", () => {
    for (const entry of fixture<Cases<{ body: string; expected: string[] }>>("tags").cases) {
      expect(mockTags(entry.body), JSON.stringify(entry.body)).toEqual(entry.expected);
    }
  });

  it("task-items: toggling the Nth item", () => {
    type Entry = { body: string; index: number; checked: boolean; expected?: string; error?: string };
    for (const entry of fixture<Cases<Entry>>("task-items").cases) {
      const got = outcome(() => toggleTaskItem(entry.body, entry.index, entry.checked));
      expect(got, JSON.stringify(entry)).toEqual(entry.error ? { error: entry.error } : entry.expected);
    }
  });

  it("forest-stage: the growth stage of a day", () => {
    for (const entry of fixture<Cases<{ notes: [number, number, number?][]; stage: number }>>("forest-stage").cases) {
      const notes = entry.notes.flatMap(([len, files, times]) => Array.from({ length: times ?? 1 }, () => ({ len, files })));
      expect(mockStage(notes.map((n) => n.len), notes.map((n) => n.files)), JSON.stringify(entry)).toBe(entry.stage);
    }
  });

  it("forest-day-chat: the chat a click on a day opens", () => {
    type Entry = { counts: [string, number][]; current: string | null; expected: string | null };
    // The fixture writes the Inbox as `inbox`: the mock knows its real id.
    const real = (id: string | null) => (id === "inbox" ? INBOX_ID : id);
    for (const entry of fixture<Cases<Entry>>("forest-day-chat").cases) {
      const counts = entry.counts.map(([id, count]) => [real(id) ?? id, count] as const);
      expect(dayChat(counts, real(entry.current)), JSON.stringify(entry)).toBe(real(entry.expected));
    }
  });

  it("attachment-kind: what the name of a file makes of it", () => {
    type Entry = { name: string; imageFormat?: string; kind: string; mime?: string };
    for (const entry of fixture<Cases<Entry>>("attachment-kind").cases) {
      expect(attachmentKind(entry.name), JSON.stringify(entry)).toBe(entry.kind);
      const tile = mockAttachment("t", entry.name);
      expect(tile.kind, JSON.stringify(entry)).toBe(entry.kind);
      if (entry.mime) expect(tile.mime, JSON.stringify(entry)).toBe(entry.mime);
    }
  });
});
