import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Frame, FrameLabel } from "../stories/Frame";
import { Button } from "./Button";
import { Checkbox } from "./Checkbox";
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

/** Фокус с клавиатуры: кольцо фокуса видно на кнопке, иконке-кнопке и поле. */
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

/** Флажок (слайс 4): пустой, отмеченный, недоступный, с описанием и с длинным именем; полоса прогресса. */
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

/** Фокус с клавиатуры на флажке: кольцо вокруг квадрата. */
export const ФокусНаФлажке: Story = {
  render: () => (
    <Frame>
      <Checkbox autoFocus defaultChecked>
        Избранное
      </Checkbox>
    </Frame>
  ),
};

/** Длинный текст и кириллица с латиницей. */
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
