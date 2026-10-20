import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Frame, FrameLabel } from "../stories/Frame";
import { Button } from "./Button";
import { IconButton } from "./IconButton";
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
