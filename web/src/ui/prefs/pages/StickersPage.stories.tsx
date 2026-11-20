import type { Meta, StoryObj } from "@storybook/react-vite";
import { Frame } from "../../stories/Frame";
import { StickersPage, type TokenControls } from "./StickersPage";
import "../prefs.css";

// Settings → Stickers (slice 5c): the bot token row. The token is typed, sent, and never shown again.
const noop = () => undefined;
const base: TokenControls = { saved: false, busy: false, error: null, itemLeft: false, onSave: noop, onRemove: noop };

const meta = {
  title: "Настройки/Стикеры: токен бота",
  component: StickersPage,
  args: base,
  decorators: [
    (Story) => (
      <Frame surface="paper">
        <div className="prefs__column">
          <Story />
        </div>
      </Frame>
    ),
  ],
} satisfies Meta<typeof StickersPage>;

export default meta;
type Story = StoryObj<typeof meta>;

/** No token yet: the explanation, a password-style field with Save, the two notes. Until a token is saved, Kinige makes no request at all. */
export const Пусто: Story = {};

/** "Token saved" with Remove replaces the field. */
export const Сохранён: Story = { args: { saved: true } };

/** The flag is being read: the explanation and the notes, no field yet. */
export const Читается: Story = { args: { saved: null } };

/** Saving is under way. */
export const Сохраняется: Story = { args: { busy: true } };

export const ОшибкаФормат: Story = { args: { error: "telegram_token_invalid" } };

export const ОшибкаСвязкаКлючей: Story = { args: { error: "secret_store_unavailable" } };

/** macOS asked for access to the saved token and it was not given: the text says to allow the request macOS shows and to try again. */
export const ОшибкаДоступ: Story = { args: { saved: true, error: "secret_store_denied" } };

/** "Remove" was pressed and macOS refused to delete the old item: the flag is off, so the field is back, and one line says where the old token is. */
export const СтарыйТокенОстался: Story = { args: { saved: false, itemLeft: true } };

export const СтарыйТокенОсталсяEn: Story = { args: { saved: false, itemLeft: true }, globals: { language: "en" } };

/** Windows and Linux have no secret store in this slice: saving is refused with its own line. */
export const ОшибкаНетХранилища: Story = { args: { saved: false, error: "secret_store_unsupported" } };

export const ПустоEn: Story = { globals: { language: "en" } };

export const СохранёнEn: Story = { args: { saved: true }, globals: { language: "en" } };

export const ОшибкаДоступEn: Story = { args: { saved: true, error: "secret_store_denied" }, globals: { language: "en" } };

/** Large text: the row wraps, the notes grow. */
export const КрупныйТекст: Story = { args: { error: "telegram_token_invalid" }, globals: { textScale: "large" } };

export const КрупныйТекстEn: Story = { args: { saved: true, error: "secret_store_denied" }, globals: { textScale: "large", language: "en" } };
