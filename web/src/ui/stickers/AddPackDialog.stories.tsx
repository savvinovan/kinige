import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import { AddPackDialog, type AddPackDialogProps } from "./AddPackDialog";
import { RemovePackDialog } from "./RemovePackDialog";

// The "Add a sticker pack" dialog in every state (slice 5c): no token, the form, each error line, the progress and the results; and the remove question.
const noop = () => undefined;
const LINK = "https://t.me/addstickers/TestPack";

const base: AddPackDialogProps = {
  state: { phase: "idle", error: null },
  hasToken: true,
  link: "",
  onLinkChange: noop,
  onAdd: noop,
  onCancelJob: noop,
  onClose: noop,
  onOpenSettings: noop,
};

const meta = {
  title: "Компоненты/Стикеры: добавление набора",
  component: AddPackDialog,
  args: base,
} satisfies Meta<typeof AddPackDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

const error = (code: ErrorCode): Story => ({ args: { link: LINK, state: { phase: "idle", error: code } } });

/** The form: a field for the link, Add (waits for a link) and Cancel. */
export const Пустая: Story = {};

/** The link is typed: Add is ready. */
export const ССылкой: Story = { args: { link: LINK } };

/** No bot token saved: one line says where to put it, "Open Settings" goes there, nothing starts. */
export const БезТокена: Story = { args: { hasToken: false } };

/** The job started and Telegram has not answered yet. */
export const Старт: Story = { args: { link: LINK, state: { phase: "running", done: null, total: null, cancelling: false } } };

/** "Downloading 12 of 30" with Cancel; no cross while the job runs. */
export const Загрузка: Story = { args: { link: LINK, state: { phase: "running", done: 12, total: 30, cancelling: false } } };

/** Cancel was pressed and the job is stopping. */
export const Отмена: Story = { args: { link: LINK, state: { phase: "running", done: 12, total: 30, cancelling: true } } };

/** The result with skipped animated and video stickers. */
export const РезультатСПропусками: Story = { args: { state: { phase: "done", result: { title: "Test Pack", added: 27, skipped: 3 } } } };

/** The result with nothing skipped. */
export const РезультатБезПропусков: Story = { args: { state: { phase: "done", result: { title: "Test Pack", added: 30, skipped: 0 } } } };

export const ОшибкаНетНабора: Story = error("sticker_pack_not_found");
export const ОшибкаСсылка: Story = error("sticker_pack_link_invalid");
export const ОшибкаТокенНеПринят: Story = error("sticker_pack_token_refused");
export const ОшибкаНетСвязи: Story = error("sticker_pack_unreachable");
export const ОшибкаПодождать: Story = error("sticker_pack_wait");
export const ОшибкаТолькоАнимация: Story = error("sticker_pack_only_animated");
export const ОшибкаСлишкомБольшой: Story = error("sticker_pack_too_large");
export const ОшибкаУжеГрузится: Story = error("sticker_pack_busy");
export const ОшибкаОтвет: Story = error("sticker_pack_bad_response");

/** The access to the saved token was refused (macOS): what to do about it. */
export const ОшибкаДоступКТокену: Story = error("secret_store_denied");

export const Английский: Story = { args: { link: LINK, state: { phase: "idle", error: "sticker_pack_not_found" } }, globals: { language: "en" } };

export const ЗагрузкаАнглийский: Story = { args: { state: { phase: "running", done: 12, total: 30, cancelling: false } }, globals: { language: "en" } };

export const РезультатАнглийский: Story = {
  args: { state: { phase: "done", result: { title: "Test Pack", added: 27, skipped: 3 } } },
  globals: { language: "en" },
};

export const БезТокенаАнглийский: Story = { args: { hasToken: false }, globals: { language: "en" } };

/** Large text: the field, the buttons and the one-line errors wrap and grow, nothing is cut. */
export const КрупныйТекст: Story = { args: { link: LINK, state: { phase: "idle", error: "sticker_pack_token_refused" } }, globals: { textScale: "large" } };

export const КрупныйТекстЗагрузка: Story = {
  args: { state: { phase: "running", done: 12, total: 30, cancelling: false } },
  globals: { textScale: "large", language: "en" },
};

// ---- the remove question ----

type RemoveStory = StoryObj<typeof RemovePackDialog>;

/** "Remove «Test Pack»? The stickers you sent stay in your entries." */
export const УдалитьНабор: RemoveStory = {
  render: () => <RemovePackDialog pack={{ source: "telegram_link", title: "Test Pack" }} onConfirm={noop} onClose={noop} />,
};

/** The pack from the export is named in the interface language. */
export const УдалитьНаборИзTelegram: RemoveStory = {
  render: () => <RemovePackDialog pack={{ source: "telegram_export", title: "From Telegram" }} onConfirm={noop} onClose={noop} />,
  globals: { language: "en" },
};

export const УдалитьОшибка: RemoveStory = {
  render: () => <RemovePackDialog pack={{ source: "telegram_link", title: "Test Pack" }} error="store_write_failed" onConfirm={noop} onClose={noop} />,
};
