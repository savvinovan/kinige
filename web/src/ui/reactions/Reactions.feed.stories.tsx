import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import type { ReactionView } from "../../api/generated/ReactionView";
import { withScene } from "../attachments/AttachmentsStage";
import { Message } from "../message/Message";
import "../feed/feed.css";
import { ReactionChoicesProvider } from "./ReactionsContext";

// Slice 5a: the hover bar next to a full-width bubble, in the real feed's structure (a `column-reverse` scroller with the 668 px column). The bar does not fit
// to the right of such a bubble, so it stands over its top edge; a put or a removal grows the bubble upwards, and `tests/visual/reactions.spec.ts` checks that
// the pressed button stays under the pointer, with the feed at its end and scrolled to the middle.

const HEART = "❤\u{FE0F}";
const SENT = new Date(2026, 10, 18, 9, 5);
const PUT = Date.UTC(2026, 10, 18, 9, 30);
const noop = () => undefined;
const LONG =
  "Сегодня долго думал о том, как устроен год: он состоит из одинаковых дней, и всё же каждый из них запоминается по-своему. " +
  "Хочется записывать не события, а то, что осталось после них, чтобы потом перечитывать как книгу и узнавать себя.";

const meta = {
  title: "Компоненты/Реакции в ленте",
  decorators: [
    withScene,
    (Story) => (
      <ReactionChoicesProvider value={{ quick: [HEART, "😂", "😢"], recent: [] }}>
        <Story />
      </ReactionChoicesProvider>
    ),
  ],
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Item({ text, target = false }: { text: string; target?: boolean }) {
  const [reactions, setReactions] = useState<ReactionView[]>([]);
  const toggle = (emoji: string, on: boolean) =>
    setReactions((list) => (on ? [...list, { emoji, putAt: PUT, putTz: "UTC", yearsLater: 0 }] : list.filter((r) => r.emoji !== emoji)));
  return (
    <li className="feed__item" data-target={target ? "yes" : undefined}>
      <Message markdown={text} sentAt={SENT} timeZone="UTC" reactions={reactions} reactionKeysHad={[]} onReact={toggle} onEdit={noop} onDelete={noop} />
    </li>
  );
}

/** The feed with the full-width entry last (the feed at its end) or among others (scrolled to the middle by the test). */
function WideFeed({ before, after }: { before: number; after: number }) {
  const filler = (n: number, side: string) => Array.from({ length: n }, (_, i) => <Item key={`${side}${i}`} text={`Запись ${side} ${i + 1}: пара слов, чтобы лента была выше окна.`} />);
  return (
    <div className="feed-scroll reaction-story-feed">
      <ul className="feed">
        {filler(before, "до")}
        <Item text={LONG} target />
        {filler(after, "после")}
      </ul>
    </div>
  );
}

/** The entry is the last one: the feed lies at its end, as right after sending. */
export const ШирокаяЗаписьВКонце: Story = { render: () => <WideFeed before={5} after={0} /> };

/** The entry is among others: the test scrolls the feed so that it is in the middle of the view. */
export const ШирокаяЗаписьВСередине: Story = { render: () => <WideFeed before={5} after={5} /> };
