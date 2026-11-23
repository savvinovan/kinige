import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState, type ReactNode } from "react";
import type { ReactionView } from "../../api/generated/ReactionView";
import { mockMessage } from "../../api/mockData";
import { Bubble, withScene } from "../attachments/AttachmentsStage";
import { demoFile, demoPeaks, demoPhoto, demoVoice } from "../attachments/demo";
import { Message } from "../message/Message";
import { OnThisDayCard } from "../onthisday/OnThisDayCard";
import { CARD_TODAY, detailedYear } from "../onthisday/demoCard";
import { NO_FILTERS, SearchResults } from "../search/SearchResults";
import { TrashView } from "../trash/TrashView";
import { ReactionChoicesProvider } from "./ReactionsContext";
import { ReactionPicker } from "./ReactionPicker";

// Slice 5a: the hover bar with the reaction buttons (none, one pressed, ten, after 32), the chips under the text in every kind of bubble, the popover of the
// card, the read-only places. The time is fixed so that snapshots do not change from run to run.

const HEART = "❤\u{FE0F}";
const SENT = new Date(2026, 10, 18, 9, 5);
/** A moment a few days after the entry, in UTC: the chip's tooltip is not shown, but the data is the real shape. */
const PUT = Date.UTC(2026, 10, 18, 9, 30);
const chip = (emoji: string, yearsLater = 0): ReactionView => ({ emoji, putAt: PUT, putTz: "UTC", yearsLater });
const DIFFERENT = Array.from({ length: 10 }, (_, i) => String.fromCodePoint(0x1f600 + i));
const noop = () => undefined;
const live = { onEdit: noop, onToggleTask: noop, onToggleFavorite: noop, onChangeDate: noop, onDelete: noop, onReact: noop };

const meta = {
  title: "Компоненты/Реакции",
  decorators: [
    withScene,
    (Story) => (
      <ReactionChoicesProvider value={{ quick: [HEART, "😂", "😢"], recent: ["🔥", HEART, "🥲"] }}>
        <Story />
      </ReactionChoicesProvider>
    ),
  ],
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const message = (extra: Partial<React.ComponentProps<typeof Message>> = {}, text = "Сдал экзамен!") => (
  <Message markdown={text} sentAt={SENT} timeZone="UTC" {...live} {...extra} />
);

/** The bar: three quick emoji, "Add reaction", a thin divider and then the usual buttons. No reaction yet. */
export const РядБезРеакций: Story = { render: () => message({ showActions: true }) };

/** One reaction is on: its button is pressed (a quiet fill) and the chip stands under the text with the time at the end of its row. */
export const РядОднаНажата: Story = { render: () => message({ showActions: true, reactions: [chip(HEART)], reactionKeysHad: ["❤"] }) };

/** Ten live reactions: the buttons that would add are muted and say why; the pressed one still removes. */
export const РядДесятьРеакций: Story = {
  render: () =>
    message(
      { showActions: true, reactions: [chip(HEART), ...DIFFERENT.slice(0, 9).map((e) => chip(e))], reactionKeysHad: ["❤", ...DIFFERENT.slice(0, 9)] },
      "Десять реакций на одной записи",
    ),
};

/** After 32 different emoji the entry took no new one: the crying face is new, so it is muted; the heart and the laughing face were used and still toggle. */
export const РядПослеТридцатиДвух: Story = {
  render: () => message({ showActions: true, reactionKeysHad: [...Array.from({ length: 30 }, (_, i) => String.fromCodePoint(0x1f300 + i)), "❤", "😂"] }),
};

/** The same bar in English. */
export const РядАнглийский: Story = { globals: { language: "en" }, render: () => message({ showActions: true, reactions: [chip("😂")] }, "Passed the exam!") };

/** One chip. */
export const ЧипОдин: Story = { render: () => message({ reactions: [chip(HEART)] }) };

/** An entry whose text is a single emoji (the user's example): the chips must not look like more of the text: they are pills with a gap above them. */
export const ТекстОдинЭмодзи: Story = {
  render: () => message({ reactions: [chip(HEART), chip("🔥"), chip("😂")], reactionKeysHad: ["❤", "🔥", "😂"] }, "🎉"),
};

/** Ten chips wrap inside the bubble, the time at the end of the last row. */
export const ЧипыДесять: Story = { render: () => message({ reactions: DIFFERENT.map((e) => chip(e)) }, "Короткая запись") };

/** A reaction put a year after the entry carries a quiet "1 г." (the tooltip says "1 год спустя"). */
export const ПозднийЧип: Story = { render: () => message({ reactions: [chip("🥲", 1), chip(HEART, 3)] }) };

/** A long text: the chips are under its last line, left-aligned. */
export const ДлинныйТекст: Story = {
  render: () =>
    message(
      { reactions: [chip(HEART), chip("🔥")] },
      "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения.",
    ),
};

/** An album with no text: the pictures fill the bubble, the chips sit under them and the time is at the end of their row, not a pill on the picture. */
export const Альбом: Story = {
  render: () => (
    <Bubble
      attachments={[demoPhoto("r1", "IMG_0001.jpg", 1280, 960), demoPhoto("r2", "IMG_0002.jpg", 960, 1280)]}
      extra={{ reactions: [chip(HEART), chip("🔥")], onReact: noop }}
    />
  ),
};

/** A voice note with no text. */
export const Голосовое: Story = {
  render: () => <Bubble attachments={[demoVoice("v1", 40_000, { waveform: demoPeaks(3) })]} timeBeside extra={{ reactions: [chip("🙏")], onReact: noop }} />,
};

/** A file with text. */
export const Файл: Story = {
  render: () => <Bubble attachments={[demoFile("f1", "menu.pdf", "file", 120_000)]} text="Меню на неделю" extra={{ reactions: [chip(HEART)], onReact: noop }} />,
};

/** An entry from a newer version (read-only): the chips are plain labels, the reaction buttons of the bar are muted. */
export const ТолькоЧтение: Story = { render: () => message({ showActions: true, readOnly: true, reactions: [chip(HEART)] }) };

/** The entry whose reaction group is not well-formed shows nothing: no chips, no error (the interface gets an empty list). */
export const ГруппаИспорчена: Story = { render: () => message({ reactions: [] }, "Запись, у которой группа реакций повреждена") };

/** The popover of the "On this day" card: the three quick emoji and "More". */
function OpenCardPopover() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    host.current?.querySelector<HTMLButtonElement>(".on-this-day__react")?.click();
  }, []);
  const year = detailedYear(2025, ["Сдал экзамен!"]);
  const first = year.entries[0];
  if (first) first.message = mockMessage({ ...first.message, reactions: [chip("🥲", 1)], reactionKeysHad: ["🥲"] });
  return (
    <div ref={host} className="story-column">
      <OnThisDayCard card={{ today: CARD_TODAY, years: [year] }} onOpen={noop} onReply={noop} onDismiss={noop} onReact={noop} />
    </div>
  );
}
export const КарточкаСОкном: Story = { render: () => <OpenCardPopover /> };

/** A 24-pixel square at (40, 40) in the stage: where the picker's opener would be (a rectangle in window pixels, not a style). */
const STORY_ANCHOR = anchorAt(40, 24);

function anchorAt(origin: number, size: number) {
  return { left: origin, top: origin, right: origin + size, bottom: origin + size };
}

/** The picker in reaction mode: the recent reactions on top. */
function Full({ children }: { children: ReactNode }) {
  return <div className="reaction-story-stage">{children}</div>;
}
export const ВыборПолный: Story = {
  render: () => (
    <Full>
      <ReactionPicker
        anchor={STORY_ANCHOR}
        start="full"
        quick={[HEART, "😂", "😢"]}
        recent={["🔥", HEART, "🥲"]}
        state={{ reactions: [], keysHad: [] }}
        language="ru"
        onReact={noop}
        onClose={noop}
      />
    </Full>
  ),
};

/** The corner of the trash: the chips show, nothing can be changed. */
export const Корзина: Story = {
  render: () => (
    <TrashView
      items={[
        {
          message: mockMessage({ id: "t1", bodyMd: "Сдал экзамен!", journalDate: "2026-11-18", reactions: [chip(HEART), chip("🔥")], reactionKeysHad: ["❤", "🔥"] }),
          chatTitle: "",
          chatIcon: "📥",
          chatIsInbox: true,
          chatColor: "grey",
          deletedAt: Date.UTC(2026, 10, 19),
          daysLeft: 28,
        },
      ]}
      language="ru"
      today="2026-11-20"
      onRestore={noop}
      onPurge={noop}
    />
  ),
};

/** A search row with its chips under the snippet. */
export const СтрокаПоиска: Story = {
  render: () => (
    <SearchResults
      listId="r"
      query="экзамен"
      chats={[]}
      hits={[
        {
          message: mockMessage({ id: "s1", bodyMd: "Сдал экзамен!", journalDate: "2026-11-18", reactions: [chip(HEART), chip("🔥")], reactionKeysHad: ["❤", "🔥"] }),
          chatTitle: "",
          chatIcon: "📥",
          chatIsInbox: true,
          chatColor: "grey",
          snippet: "Сдал экзамен!",
          terms: ["экзамен"],
        },
      ]}
      hasMore={false}
      loading={false}
      searched
      today="2026-11-20"
      language="ru"
      filters={NO_FILTERS}
      onFiltersChange={noop}
      canScopeChat
      onPickDate={noop}
      activeIndex={0}
      onActiveChange={noop}
      onOpenChat={noop}
      onOpenHit={noop}
      onLoadMore={noop}
    />
  ),
};

/**
 * "No jump" (step 2 of the scenario): a click on an emoji of the bar adds a row of chips, the bubble grows upwards, and the bar, which is
 * level with the bubble's bottom line, stays where it was. The feed is `column-reverse`, so with the scroll offset kept the bottom edge holds.
 * The story stands the bubble at the bottom of a column-reverse box, as the feed does; `tests/visual/reactions.spec.ts` clicks the heart and
 * measures the bar before and after.
 */
function Feedlike() {
  const [reactions, setReactions] = useState<ReactionView[]>([]);
  return (
    <div className="story-column reaction-story-column">
      <div>
        {message({ showActions: true, reactions, onReact: (emoji, on) => setReactions((list) => (on ? [...list, chip(emoji)] : list.filter((r) => r.emoji !== emoji))) })}
      </div>
    </div>
  );
}

export const БезСкачка: Story = { render: () => <Feedlike /> };
