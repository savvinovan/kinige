import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import { Frame } from "../stories/Frame";
import { ProfileMenu } from "./ProfileMenu";
import "./profile-stories.css";

interface DemoProps {
  name?: string;
  statusDot?: boolean;
  defaultOpen?: boolean;
  /** Focuses the profile button: this is how the story shows the focus ring without a mouse. */
  focus?: boolean;
  /** Collapsed sidebar: only the avatar; the menu opens to the right. */
  compact?: boolean;
}

/** The profile row on the sidebar background, at the bottom edge, as in the window: the menu opens upward (to the right when collapsed). */
function Demo({ name, statusDot, defaultOpen, focus = false, compact = false }: DemoProps) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // An open menu focuses the first item itself; the button is focused only in the "Фокус" story.
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

/** Default: the avatar "Я", the name "Я", the chevron. */
export const Обычное: Story = {};

/** The menu is open: "Настройки", "Экспорт", "О программе" are visual only, the items are muted (aria-disabled, "Появится позже"). */
export const МенюОткрыто: Story = { args: { defaultOpen: true } };

/** A two-word name: the initials "АС". */
export const ИмяИзДвухСлов: Story = { args: { name: "Александр Саввинов" } };

/** A long name is truncated with an ellipsis, the avatar and chevron stay in place; Cyrillic and Latin. */
export const ДлинноеИмя: Story = {
  args: { name: "Александр Саввинов-Очень-Длинная-Фамилия and Some Long English Name" },
};

/** The status dot by the avatar (slice 11): not shown in the D1 app, the prop is ready. */
export const СТочкойСтатуса: Story = { args: { statusDot: true } };

/** Keyboard focus on the profile button. */
export const Фокус: Story = { args: { focus: true } };

/** Collapsed sidebar: only the avatar, centered in the rail; the screen reader reads the name from the button label. */
export const Компактный: Story = { args: { compact: true } };

/** Collapsed sidebar, menu open: it extends to the right of the avatar, over the feed. */
export const КомпактныйМенюОткрыто: Story = { args: { compact: true, defaultOpen: true } };

/** Collapsed sidebar, keyboard focus on the avatar. */
export const КомпактныйФокус: Story = { args: { compact: true, focus: true } };
