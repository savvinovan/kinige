import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import { demoEntry } from "../../api/demoData";
import { staticVoice } from "../../voice/staticVoice";
import type { VoiceState } from "../../voice/useVoiceRecorder";
import { EmojiStoryData } from "../emoji/EmojiStoryData";
import type { FeedMessage } from "../feed/buildFeed";
import { storyLevels } from "../voice/storyLevels";
import { CapturePanel } from "./CapturePanel";
import "./capture-stories.css";

const TODAY = "2026-10-20";

const ONE = [demoEntry("a", TODAY, "09:12", "Идея: журнал, в котором год виден как лес.")];

const THREE = [
  demoEntry("a", TODAY, "09:12", "Идея: журнал, в котором год виден как лес."),
  demoEntry("b", TODAY, "11:30", "### Планы на выходные\n- [x] Поход в горы\n- [ ] Разобрать фотографии"),
  demoEntry("c", TODAY, "13:45", "Прочитал [The Psychology of Habits](https://fs.blog/habits) и записал вывод: **привычка — это петля**."),
];

const CODE = ["```", "fn main() {", "    let today = Day::new(2026, 10, 20);", '    println!("Записей: {}", today.count());', "}", "```"].join("\n");

const LONG = [
  demoEntry(
    "a",
    TODAY,
    "09:12",
    "Проснулся рано и до завтрака записал мысль про журнал: пишешь как в мессенджере, перечитываешь как книгу. Главное — не выбирать, куда писать. Остальное потом, когда будет время всё разобрать и разложить по чатам; сейчас важно не потерять саму мысль, пока она тёплая. Дальше ещё несколько строк, чтобы запись точно не поместилась.",
  ),
  demoEntry("b", TODAY, "11:30", `Починил историю, вот что получилось:\n\n${CODE}`),
  demoEntry(
    "c",
    TODAY,
    "13:45",
    "Список покупок на неделю:\n- хлеб\n- молоко\n- яблоки\n- кофе\n- Сверхъестественнонепроизносимоесловобезпробеловкотороедлиннеестрокиокна\n- ещё одна строка",
  ),
];

interface DemoProps {
  context: readonly FeedMessage[];
  appendTargetId?: string | null;
  value?: string;
  error?: string;
  narrow?: boolean;
  /** Focus on the "Send" button: this is how the story shows the keyboard focus ring. */
  focusSend?: boolean;
  /** The emoji picker is open on show: the panel in the flow above the field, the window taller. */
  pickerOpen?: boolean;
  /** The microphone (KINIG-153), frozen in this state; without it the story has no microphone button, as the first stories of the slice. */
  voice?: VoiceState;
}

/** The capture window at its own size on a "desk"; the text in the field is live, Enter clears it, as after sending. */
function Demo({ context, appendTargetId = null, value: initial = "", error, narrow = false, focusSend = false, pickerOpen = false, voice }: DemoProps) {
  const [value, setValue] = useState(initial);
  const [open, setOpen] = useState(pickerOpen);
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // The parent effect runs after the child's: the story's focus overrides the field's autofocus.
    if (focusSend) host.current?.querySelector<HTMLButtonElement>(".icon-button--accent")?.focus();
  }, [focusSend]);
  return (
    <EmojiStoryData wait={pickerOpen}>
      <div className="story-capture-desk" ref={host}>
        <div className={`story-capture-window${narrow ? " story-capture-window--narrow" : ""}`}>
          <CapturePanel
            context={context}
            appendTargetId={appendTargetId}
            value={value}
            onChange={setValue}
            onSend={() => setValue("")}
            onAppend={() => setValue("")}
            onHide={() => undefined}
            error={error}
            emoji={{
              recent: ["📚", "💡", "🍳", "🧭"],
              onPicked: () => undefined,
              open,
              onOpenChange: setOpen,
            }}
            voice={voice ? staticVoice(voice, { levels: storyLevels }) : undefined}
          />
        </div>
      </div>
    </EmojiStoryData>
  );
}

const meta = {
  title: "Компоненты/Окно захвата",
  component: Demo,
  parameters: { layout: "fullscreen" },
  args: { context: THREE },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** No entries today: only the field, a single row (a "pill" without an empty area on top). */
export const ПустоЗаСегодня: Story = { args: { context: [] } };

/** One entry: a pale bubble with the time above the field. */
export const ОднаЗапись: Story = { args: { context: ONE } };

/** Three entries (a heading with tasks, a link, bold): a time on each, the newest closer to the field; the link is not clickable. */
export const ТриЗаписи: Story = {};

/** There is an append target: under the field the hint "⌘Enter -- append" (the window grows downward, the field stays in place). */
export const ЕстьЦельДописывания: Story = { args: { appendTargetId: "c" } };

/** There is a target and the field has text: a round "Send" button on the right inside the field, the hint under the field. */
export const ЕстьЦельИТекст: Story = { args: { appendTargetId: "c", value: "и ещё одна мысль" } };

/** No target (the last entry is older than 10 minutes or not our own): no hint, no row under the field. */
export const НетЦелиДописывания: Story = { args: { appendTargetId: null } };

/** Long entries: clipped by lines with an ellipsis; a code block, a list and a word without spaces do not break the bubble. */
export const ДлинныеЗаписи: Story = { args: { context: LONG, appendTargetId: "c" } };

/** Long text in the field: the window grows to its height limit, the field to 45% of the panel (beyond that it scrolls inside), old bubbles go beyond the top edge. */
export const ДлинныйТекстВПоле: Story = {
  args: {
    appendTargetId: "c",
    value: Array.from({ length: 14 }, (_, i) => `Строка ${i + 1}: пишешь как в мессенджере, перечитываешь как книгу.`).join("\n"),
  },
};

/** A very long word without spaces (for example a pasted link or data): wrapped by characters, no horizontal scrolling. */
export const ДлинноеСловоВПоле: Story = {
  args: { appendTargetId: "c", value: `прежний черновик\n\n${"я".repeat(900)}` },
};

/** An error above the field: space for it is reserved in advance, the text stayed in the field. */
export const Ошибка: Story = {
  args: {
    appendTargetId: "c",
    value: "и ещё одна мысль",
    error: "Не удалось сохранить запись. Текст остался в поле.",
  },
};

/** Keyboard focus on the "Send" button (in the field, focus shows as a thin line by itself). */
export const ФокусСКлавиатуры: Story = { args: { appendTargetId: "c", value: "и ещё", focusSend: true } };

/** Narrow width: bubbles, the hint and the error wrap, the field does not overflow. */
export const УзкаяШирина: Story = {
  args: {
    context: LONG,
    appendTargetId: "c",
    narrow: true,
    value: "и ещё",
    error: "Не удалось сохранить запись. Текст остался в поле.",
  },
};

/**
 * The emoji picker is open: the panel sits in the flow between the context and the field, the window grew upward (the bottom
 * edge and the field stay in place), "Недавние" (Recent) on top. With the picker open the field shows at most three lines.
 */
export const ПикерЭмодзи: Story = { args: { appendTargetId: "c", pickerOpen: true } };

/** The picker is open, the field has long text: the field is limited to three lines with scrolling inside, the picker is intact, old bubbles go beyond the edge. */
export const ПикерЭмодзиИДлинныйТекст: Story = {
  args: {
    appendTargetId: "c",
    pickerOpen: true,
    value: Array.from({ length: 8 }, (_, i) => `Строка ${i + 1}: пишешь как в мессенджере, перечитываешь как книгу.`).join("\n"),
  },
};

/** The picker is open on an empty day: only the picker and the field. */
export const ПикерЭмодзиПустойДень: Story = { args: { context: [], pickerOpen: true } };

/** English: the emoji picker is open. */
export const EnПикерЭмодзи: Story = { args: { context: ONE, appendTargetId: "a", pickerOpen: true }, globals: { language: "en" } };

/** English: three entries, the hint "⌘Enter -- append". */
export const EnЦельДописывания: Story = {
  args: {
    context: [
      demoEntry("a", TODAY, "09:12", "Idea: a journal where the year looks like a forest."),
      demoEntry("b", TODAY, "11:30", "### Weekend plans\n- [x] Hike\n- [ ] Sort the photos"),
      demoEntry("c", TODAY, "13:45", "Read [The Psychology of Habits](https://fs.blog/habits): **a habit is a loop**."),
    ],
    appendTargetId: "c",
  },
  globals: { language: "en" },
};

/** English: nothing today. */
export const EnПусто: Story = { args: { context: [] }, globals: { language: "en" } };

/** English: an error. */
export const EnОшибка: Story = {
  args: { context: ONE, value: "and one more thing", error: "Could not save the entry. The text is still in the field." },
  globals: { language: "en" },
};

// ---- The microphone (KINIG-153): the main window's button and recording bar in the capture panel. Every story also shoots at large text. ----

/** An empty field: the microphone button where the main window has it, right of the emoji button. */
export const МикрофонПустоеПоле: Story = { args: { appendTargetId: "c", voice: { kind: "idle" } } };

/** Recording at 0:12: the bar stands in for the field at the same height, the trash on the left, Send on the right; the window does not move. */
export const МикрофонЗапись: Story = { args: { appendTargetId: "c", voice: { kind: "recording", elapsedMs: 12_000 } } };

/** English, recording, with the context above. */
export const EnМикрофонЗапись: Story = {
  args: { context: ONE, appendTargetId: "a", voice: { kind: "recording", elapsedMs: 83_000 } },
  globals: { language: "en" },
};

/** A recording over five seconds asks before it is dropped: the question and two choices have to fit the panel's width in Russian too. */
export const МикрофонВопросОбУдалении: Story = { args: { voice: { kind: "confirming", elapsedMs: 12_000 } } };

/** The window was hidden mid-recording: what was recorded waits, and it says so without blaming the device. */
export const МикрофонЗаписьОстановлена: Story = { args: { voice: { kind: "lost", elapsedMs: 12_000, cause: "hidden" } } };

/** The first use, access is off in the system settings: one line above the field with "Open settings", the field is where it was. */
export const МикрофонДоступВыключен: Story = { args: { appendTargetId: "c", voice: { kind: "notice", reason: "denied" } } };

/** The main window is recording: the one microphone is taken, so this window says "unavailable" and starts nothing. */
export const EnМикрофонЗанят: Story = {
  args: { context: ONE, voice: { kind: "notice", reason: "busy" } },
  globals: { language: "en" },
};
