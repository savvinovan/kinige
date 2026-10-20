import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import { DEMO_CHATS } from "../../api/demoData";
import { Frame } from "../stories/Frame";
import { Sidebar, type SidebarChat } from "./Sidebar";

interface DemoProps {
  chats?: readonly SidebarChat[];
  selectedId?: string;
  /** Ставит фокус на выбранную строку: так история показывает кольцо фокуса без мыши. */
  focus?: boolean;
}

function Demo({ chats = DEMO_CHATS, selectedId = "inbox", focus = false }: DemoProps) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focus) host.current?.querySelector<HTMLButtonElement>("[aria-current]")?.focus();
  }, [focus]);
  return (
    <Frame surface="background">
      <div ref={host}>
        <Sidebar chats={chats} selectedId={selectedId} />
      </div>
    </Frame>
  );
}

const meta = {
  title: "Компоненты/Сайдбар",
  component: Demo,
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Обычное: поиск, пять чатов «эмодзи + имя», «Входящие» выбран. */
export const Обычное: Story = {};

/** Другой выбранный чат. */
export const ВыбранДругойЧат: Story = { args: { selectedId: "books" } };

/** Длинные имена обрезаются многоточием (полное имя — во всплывающей подсказке), эмодзи не сжимается. */
export const ДлинныеИмена: Story = {
  args: {
    chats: [
      { id: "a", name: "Подготовка к собеседованию по системному дизайну и распределённым системам", emoji: "🧭" },
      { id: "b", name: "Сверхъестественнонепроизносимоесловобезединогопробела", emoji: "🧪" },
      { id: "c", name: "Reading list: papers, blogs and long-form essays", emoji: "📖" },
      { id: "d", name: "Идеи", emoji: "💡" },
    ],
    selectedId: "a",
  },
};

/** Много чатов: список прокручивается, поиск остаётся наверху. */
export const МногоЧатов: Story = {
  args: {
    chats: Array.from({ length: 40 }, (_, i) => ({ id: `c${i}`, name: `Чат ${i + 1}`, emoji: "📌" })),
    selectedId: "c0",
  },
};

/** Фокус с клавиатуры на строке чата. */
export const Фокус: Story = { args: { focus: true } };
