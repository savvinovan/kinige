import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import { demoEntry } from "../../api/demoData";
import type { FeedMessage } from "../feed/buildFeed";
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
  /** Фокус на кнопке «Отправить»: так история показывает кольцо фокуса с клавиатуры. */
  focusSend?: boolean;
}

/** Окно захвата своего размера на «столе»; текст в поле живой, Enter очищает его, как после отправки. */
function Demo({ context, appendTargetId = null, value: initial = "", error, narrow = false, focusSend = false }: DemoProps) {
  const [value, setValue] = useState(initial);
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Родительский эффект идёт после дочернего: фокус из истории перекрывает автофокус поля.
    if (focusSend) host.current?.querySelector<HTMLButtonElement>(".icon-button--accent")?.focus();
  }, [focusSend]);
  return (
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
        />
      </div>
    </div>
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

/** За сегодня записей нет: спокойная пустая область с одной тихой строкой, поле внизу. */
export const ПустоЗаСегодня: Story = { args: { context: [] } };

/** Одна запись: бледная плашка со временем над полем. */
export const ОднаЗапись: Story = { args: { context: ONE } };

/** Три записи (заголовок и задачи, ссылка, жирный): время у каждой, самая новая ближе к полю; ссылка не нажимается. */
export const ТриЗаписи: Story = {};

/** Есть цель дописывания: у последней плашки подсказка «⌘Enter — дописать». */
export const ЕстьЦельДописывания: Story = { args: { appendTargetId: "c" } };

/** Цели нет (последняя запись старше 10 минут или не своя): подсказки нет, место под неё занято, ничего не прыгает. */
export const НетЦелиДописывания: Story = { args: { appendTargetId: null } };

/** Длинные записи: обрезаны по строкам с многоточием; блок кода, список и слово без пробелов не ломают плашку. */
export const ДлинныеЗаписи: Story = { args: { context: LONG, appendTargetId: "c" } };

/** Длинный текст в поле: поле растёт до 40% высоты окна, старые плашки уходят за верхний край, подсказка и ошибка не двигают поле. */
export const ДлинныйТекстВПоле: Story = {
  args: {
    appendTargetId: "c",
    value: Array.from({ length: 14 }, (_, i) => `Строка ${i + 1}: пишешь как в мессенджере, перечитываешь как книгу.`).join("\n"),
  },
};

/** Ошибка над полем: место для неё занято заранее, текст остался в поле. */
export const Ошибка: Story = {
  args: {
    appendTargetId: "c",
    value: "и ещё одна мысль",
    error: "Не удалось сохранить запись. Текст остался в поле.",
  },
};

/** Фокус с клавиатуры на кнопке «Отправить» (в поле фокус виден тонкой линией сам). */
export const ФокусСКлавиатуры: Story = { args: { appendTargetId: "c", value: "и ещё", focusSend: true } };

/** Узкая ширина: плашки, подсказка и ошибка переносятся, поле не вылезает. */
export const УзкаяШирина: Story = {
  args: {
    context: LONG,
    appendTargetId: "c",
    narrow: true,
    value: "и ещё",
    error: "Не удалось сохранить запись. Текст остался в поле.",
  },
};

/** Английский: три записи, подсказка «⌘Enter — append». */
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

/** Английский: пусто за сегодня. */
export const EnПусто: Story = { args: { context: [] }, globals: { language: "en" } };

/** Английский: ошибка. */
export const EnОшибка: Story = {
  args: { context: ONE, value: "and one more thing", error: "Could not save the entry. The text is still in the field." },
  globals: { language: "en" },
};
