import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import { Frame } from "../stories/Frame";
import { ProfileMenu } from "./ProfileMenu";
import "./profile-stories.css";

interface DemoProps {
  name?: string;
  statusDot?: boolean;
  defaultOpen?: boolean;
  /** Ставит фокус на кнопку профиля: так история показывает кольцо фокуса без мыши. */
  focus?: boolean;
  /** Свёрнутый сайдбар: только аватар, меню открывается вправо. */
  compact?: boolean;
}

/** Строка профиля на фоне сайдбара, у нижнего края, как в окне: меню открывается вверх (в свёрнутом виде — вправо). */
function Demo({ name, statusDot, defaultOpen, focus = false, compact = false }: DemoProps) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Открытое меню само ставит фокус на первый пункт; кнопку фокусируем только в истории «Фокус».
    if (focus) host.current?.querySelector<HTMLButtonElement>(".profile__button")?.focus();
  }, [focus]);
  return (
    <Frame surface="background" bottom>
      <div className={compact ? "story-profile story-profile--rail" : "story-profile"} ref={host}>
        <ProfileMenu name={name} statusDot={statusDot} defaultOpen={defaultOpen} compact={compact} />
      </div>
    </Frame>
  );
}

const meta = {
  title: "Компоненты/Профиль",
  component: Demo,
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Обычное: аватар «Я», имя «Я», шеврон. */
export const Обычное: Story = {};

/** Меню открыто: «Настройки», «Экспорт», «О программе» — только вид, пункты приглушены (aria-disabled, «Появится позже»). */
export const МенюОткрыто: Story = { args: { defaultOpen: true } };

/** Имя из двух слов: инициалы «АС». */
export const ИмяИзДвухСлов: Story = { args: { name: "Александр Саввинов" } };

/** Длинное имя обрезается многоточием, аватар и шеврон на месте; кириллица и латиница. */
export const ДлинноеИмя: Story = {
  args: { name: "Александр Саввинов-Очень-Длинная-Фамилия and Some Long English Name" },
};

/** Точка статуса у аватара (слайс 11): в приложении D1 не показывается, проп готов. */
export const СТочкойСтатуса: Story = { args: { statusDot: true } };

/** Фокус с клавиатуры на кнопке профиля. */
export const Фокус: Story = { args: { focus: true } };

/** Свёрнутый сайдбар: только аватар по центру полосы; имя читается диктором по подписи кнопки. */
export const Компактный: Story = { args: { compact: true } };

/** Свёрнутый сайдбар, меню открыто: выходит вправо от аватара, на ленту. */
export const КомпактныйМенюОткрыто: Story = { args: { compact: true, defaultOpen: true } };

/** Свёрнутый сайдбар, фокус с клавиатуры на аватаре. */
export const КомпактныйФокус: Story = { args: { compact: true, focus: true } };
