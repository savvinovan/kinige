import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Frame, FrameLabel } from "../stories/Frame";
import { Button } from "./Button";
import { Checkbox } from "./Checkbox";
import { CopyButton } from "./CopyButton";
import { IconButton } from "./IconButton";
import { ProgressBar } from "./ProgressBar";
import { TextField } from "./TextField";

function Controls() {
  const [query, setQuery] = useState("");
  return (
    <Frame>
      <FrameLabel>Кнопки: акцентная (тост) и в строке текста (сообщение)</FrameLabel>
      <div>
        <Button>Отменить</Button> <Button disabled>Недоступно</Button>
      </div>
      <p>
        Не отправилось. <Button variant="inline">Повторить</Button>
      </p>

      <FrameLabel>Иконки-кнопки: файл, эмодзи, голосовая, поиск</FrameLabel>
      <div>
        <IconButton icon="attach" label="Прикрепить файл" />
        <IconButton icon="emoji" label="Эмодзи" />
        <IconButton icon="voice" label="Записать голосовое" />
        <IconButton icon="search" label="Поиск" />
        <IconButton icon="voice" label="Записать голосовое (недоступно)" disabled />
      </div>

      <FrameLabel>Копировать значком: путь файла, конфиг подключения (после нажатия на полторы секунды галочка)</FrameLabel>
      <div>
        <CopyButton text="/Users/me/settings.json" label="Копировать путь" copiedLabel="Скопировано" failedLabel="Не удалось скопировать" />
        <CopyButton text="claude mcp add kinige" label="Копировать команду для Claude Code" copiedLabel="Скопировано" failedLabel="Не удалось скопировать" />
      </div>

      <FrameLabel>Поле: пустое, с текстом, ошибка</FrameLabel>
      <TextField label="Поиск" icon="search" placeholder="Поиск" value={query} onChange={(e) => setQuery(e.target.value)} />
      <TextField label="Поиск" icon="search" defaultValue="системный дизайн Raft" />
      <TextField label="Название чата" defaultValue="" placeholder="Название" error="Название не может быть пустым." />
    </Frame>
  );
}

const meta = {
  title: "Компоненты/Кнопки и поля",
  component: Controls,
} satisfies Meta<typeof Controls>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Состояния: Story = {};

/** Keyboard focus: the focus ring is visible on the button, the icon button and the field. */
export const Фокус: Story = {
  render: () => (
    <Frame>
      <FrameLabel>Фокус на кнопке</FrameLabel>
      <div>
        <Button autoFocus>Отменить</Button>
      </div>
      <FrameLabel>Фокус на иконке-кнопке</FrameLabel>
      <div>
        <IconButton icon="emoji" label="Эмодзи" autoFocus />
      </div>
      <FrameLabel>Фокус в поле (рамка поля)</FrameLabel>
      <TextField label="Поиск" icon="search" placeholder="Поиск" autoFocus />
    </Frame>
  ),
};

/** Checkbox (slice 4): empty, checked, unavailable, with a description and with a long name; the progress bar. */
export const ФлажокИПрогресс: Story = {
  render: () => (
    <Frame>
      <FrameLabel>Флажок: пустой, отмеченный, недоступный</FrameLabel>
      <Checkbox defaultChecked={false}>Личный чат</Checkbox>
      <Checkbox defaultChecked>Избранное</Checkbox>
      <Checkbox disabled>Недоступный</Checkbox>
      <FrameLabel>Флажок с описанием и длинным именем</FrameLabel>
      <Checkbox defaultChecked description="1 682 записи · 1 000 уже есть">
        Избранное
      </Checkbox>
      <Checkbox description="12 записей">
        Очень длинное имя собеседника, которое не помещается в одну строку окна импорта вместе с описанием
      </Checkbox>
      <FrameLabel>Полоса прогресса: 0, 3 200 из 15 000, готово</FrameLabel>
      <ProgressBar value={0} max={15000} label="Импорт" valueText="0 из 15 000" />
      <ProgressBar value={3200} max={15000} label="Импорт" valueText="3 200 из 15 000" />
      <ProgressBar value={15000} max={15000} label="Импорт" valueText="15 000 из 15 000" />
    </Frame>
  ),
};

/** A long checkbox label wraps (`checkbox--wrap`, D2) instead of being cut with an ellipsis; a chat name in a list stays on one line. */
export const ФлажокДлиннаяПодпись: Story = {
  render: () => (
    <Frame>
      <FrameLabel>Переносится: подпись настройки</FrameLabel>
      <Checkbox className="checkbox--wrap" defaultChecked description="Подключённый агент читает чаты, отмеченные ниже, и, если разрешено, пишет записи.">
        Разрешить агентам доступ к журналу и ко всем чатам, которые отмечены в списке «Видимые чаты» ниже на этой странице
      </Checkbox>
      <FrameLabel>Обрезается: название чата в списке</FrameLabel>
      <Checkbox description="12 записей">Очень длинное имя собеседника, которое не помещается в одну строку окна импорта вместе с описанием</Checkbox>
    </Frame>
  ),
  globals: { textScale: "large" },
};

/** Keyboard focus on the checkbox: a ring around the square. */
export const ФокусНаФлажке: Story = {
  render: () => (
    <Frame>
      <Checkbox autoFocus defaultChecked>
        Избранное
      </Checkbox>
    </Frame>
  ),
};

/** Long text and Cyrillic mixed with Latin. */
export const ДлинныйТекст: Story = {
  render: () => (
    <Frame narrow>
      <TextField
        label="Поиск"
        icon="search"
        defaultValue="Очень длинный поисковый запрос про системный дизайн, Raft и распределённые транзакции — distributed transactions"
      />
      <TextField label="Поиск" icon="search" defaultValue="Сверхъестественнонепроизносимоеслово_без_пробелов_и_Latin_mix" />
    </Frame>
  ),
};
