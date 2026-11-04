import { afterEach, describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, mockAttachment, mockMessage } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Attachments on the real `MainWindow` and `MockApi` (slice 7): the paperclip, a paste, a drop, the tiles, sending with and without
// text, the feed with photos and file cards, the lightbox, the errors above the field.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

function setup(language: "ru" | "en" = "ru", api = new MockApi({ now: NOW, language })) {
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language });
  return api;
}

const field = () => screen.findByRole("textbox", { name: /Новая заметка|New note/ });

afterEach(() => document.documentElement.removeAttribute("data-text-scale"));

async function pick(api: MockApi, names: string[]) {
  api.pickedFiles = names;
  fireEvent.click(await screen.findByRole("button", { name: "Прикрепить файл" }));
}

describe("MainWindow: вложения", () => {
  it("скрепка: выбранные файлы становятся плитками над полем, × убирает плитку, текст остаётся", async () => {
    const api = setup();
    fireEvent.change(await field(), { target: { value: "подпись" } });
    await pick(api, ["IMG_0001.jpg", "menu.pdf"]);

    const tiles = await screen.findByRole("list", { name: "Вложения к записи" });
    expect(within(tiles).getAllByRole("listitem")).toHaveLength(2);
    expect(await screen.findByRole("button", { name: "Убрать menu.pdf" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Убрать menu.pdf" }));
    await waitFor(() => expect(within(tiles).getAllByRole("listitem")).toHaveLength(1));
    expect(api.tilesOf(INBOX_ID).map((t) => t.filename)).toEqual(["IMG_0001.jpg"]);
    expect(await field()).toHaveValue("подпись");
  });

  it("отправка: фото и файл появляются в ленте альбомом и карточкой, поле и плитки пусты", async () => {
    const api = setup();
    await pick(api, ["IMG_0001.jpg", "IMG_0002.jpg", "menu.pdf"]);
    await screen.findByRole("list", { name: "Вложения к записи" });
    fireEvent.change(await field(), { target: { value: "обед" } });
    fireEvent.keyDown(await field(), { key: "Enter" });

    expect(await screen.findByRole("button", { name: /Фото IMG_0001\.jpg/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Фото IMG_0002\.jpg/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /menu\.pdf/ })).toBeInTheDocument();
    expect(screen.getByText("обед")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Вложения к записи" })).toBeNull();
    expect(await field()).toHaveValue("");
    expect(api.tilesOf(INBOX_ID)).toEqual([]);
  });

  it("отправка без текста: достаточно вложения, кнопка «Отправить» есть, пустая запись без вложений не уходит", async () => {
    const api = setup();
    expect(screen.queryByRole("button", { name: "Отправить" })).toBeNull();
    fireEvent.keyDown(await field(), { key: "Enter" });
    expect((await api.listMessages(INBOX_ID)).messages).toEqual([]);

    await pick(api, ["solo.png"]);
    const send = await screen.findByRole("button", { name: "Отправить" });
    fireEvent.click(send);
    expect(await screen.findByRole("button", { name: /Фото solo\.png/ })).toBeInTheDocument();
    const sent = (await api.listMessages(INBOX_ID)).messages;
    expect(sent.map((m) => m.bodyMd)).toEqual([""]);
  });

  it("ошибки отправки и выбора стоят над полем, плитки остаются", async () => {
    const api = setup();
    await pick(api, ["a.png"]);
    await screen.findByRole("list", { name: "Вложения к записи" });
    api.failNext("attachPick", "attachment_too_large");
    fireEvent.click(await screen.findByRole("button", { name: "Прикрепить файл" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("больше 2 ГБ");

    api.failNextSend = "attachment_not_found";
    fireEvent.keyDown(await field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("Этого вложения уже нет");
    expect(api.tilesOf(INBOX_ID)).toHaveLength(1);
  });

  it("одиннадцатая плитка: «не больше 10 вложений», десять остаются", async () => {
    const api = setup();
    await pick(api, Array.from({ length: 11 }, (_, i) => `f${i}.txt`));
    expect(await screen.findByRole("alert")).toHaveTextContent("не больше 10 вложений");
    expect(api.tilesOf(INBOX_ID)).toHaveLength(10);
  });

  it("вставка картинки из буфера (без текста) становится плиткой; вставка с текстом — обычная вставка", async () => {
    const api = setup();
    const area = await field();
    const image = new File([new Uint8Array([137, 80, 78, 71])], "image.png", { type: "image/png" });
    fireEvent.paste(area, { clipboardData: { files: [image], getData: () => "" } });
    await waitFor(() => expect(api.tilesOf(INBOX_ID)).toHaveLength(1));
    expect(api.calls.some((call) => call.startsWith("attachPaste"))).toBe(true);
    // "image.png" is a generic name: Rust names the file, the hint is not sent.
    expect(api.calls.find((call) => call.startsWith("attachPaste"))).toMatch(/attachPaste \S+ 4$/);

    const withText = new File([new Uint8Array([1])], "x.png", { type: "image/png" });
    fireEvent.paste(area, { clipboardData: { files: [withText], getData: () => "текст" } });
    await act(async () => undefined);
    expect(api.tilesOf(INBOX_ID)).toHaveLength(1);
  });

  it("перетаскивание: пока файлы над окном — «Отпустите, чтобы прикрепить»; бросили — плитки; папка не прикрепляется", async () => {
    const api = setup();
    await field();
    const overlay = () => document.querySelector(".drop-overlay");
    expect(overlay()).not.toHaveClass("drop-overlay--visible");
    act(() => api.emitAttachmentsDrag(true));
    expect(overlay()).toHaveClass("drop-overlay--visible");
    expect(within(overlay() as HTMLElement).getByText("Отпустите, чтобы прикрепить")).toBeInTheDocument();
    act(() => api.emitAttachmentsDrag(false));
    expect(overlay()).not.toHaveClass("drop-overlay--visible");

    act(() => void api.emitAttachmentsDropped(["photo.jpg", "Folder/", "notes.txt"]));
    await waitFor(() => expect(api.tilesOf(INBOX_ID)).toHaveLength(2));
    expect(await screen.findByRole("alert")).toHaveTextContent("Папки не прикрепляются");
  });

  it("корзина: бросок файлов не берётся, плитки не появляются", async () => {
    const api = setup();
    fireEvent.click(await screen.findByRole("button", { name: /^Корзина/ }));
    await act(async () => undefined);
    act(() => void api.emitAttachmentsDropped(["a.png"]));
    await act(async () => undefined);
    expect(api.tilesOf(INBOX_ID)).toEqual([]);
  });

  it("плитки черновика возвращаются после переключения чата и обратно", async () => {
    const api = new MockApi({ now: NOW, language: "ru", chats: [{ id: "books", title: "Книги" }] });
    setup("ru", api);
    await pick(api, ["keep.pdf"]);
    await screen.findByRole("list", { name: "Вложения к записи" });
    fireEvent.click(await screen.findByRole("button", { name: /^Книги/ }));
    await waitFor(() => expect(screen.queryByRole("list", { name: "Вложения к записи" })).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: /^Входящие/ }));
    expect(await screen.findByRole("list", { name: "Вложения к записи" })).toBeInTheDocument();
  });

  it("лента: размер коробки фото не меняется, когда миниатюра загрузилась; пропавший файл и карточки остаются", async () => {
    // A portrait photo, as after the EXIF rotation.
    const portrait = mockAttachment("p1", "IMG_0001.jpg");
    portrait.width = 600;
    portrait.height = 900;
    const api = new MockApi({
      now: NOW,
      language: "ru",
      messages: [
        mockMessage({
          id: "m1",
          bodyMd: "кадры",
          attachments: [
            portrait,
            mockAttachment("f1", "очень_длинное_название_документа_для_проверки.pdf"),
            mockAttachment("m9", "gone.jpg", { state: "missing" }),
          ],
        }),
      ],
    });
    setup("ru", api);
    const photo = await screen.findByRole("button", { name: /Фото IMG_0001\.jpg/ });
    const box = photo.closest(".album") as HTMLElement;
    const before = box.getAttribute("style");
    const image = photo.querySelector("img");
    expect(image).not.toBeNull();
    if (image) fireEvent.load(image);
    expect(box.getAttribute("style")).toBe(before);
    expect(screen.getByText("Файла нет")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /очень_длинное/ }));
    await waitFor(() => expect(api.openedAttachments).toEqual(["f1"]));
  });

  it("лайтбокс: клик по фото открывает окно со счётчиком, стрелки листают, Escape закрывает, «Открыть в программе»", async () => {
    const api = new MockApi({
      now: NOW,
      language: "ru",
      messages: [
        mockMessage({
          id: "m1",
          bodyMd: "",
          attachments: ["a.jpg", "b.jpg", "c.jpg"].map((name, i) => mockAttachment(`p${i}`, name)),
        }),
      ],
    });
    setup("ru", api);
    fireEvent.click(await screen.findByRole("button", { name: /Фото b\.jpg/ }));
    const dialog = await screen.findByRole("dialog", { name: "Просмотр фото" });
    expect(within(dialog).getByText("2 / 3")).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: "ArrowRight" });
    expect(within(dialog).getByText("3 / 3")).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: "ArrowRight" });
    expect(within(dialog).getByText("3 / 3")).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Открыть в программе по умолчанию" }));
    await waitFor(() => expect(api.openedAttachments).toEqual(["p2"]));
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Просмотр фото" })).toBeNull());
  });

  it("английский: подписи и ошибка на английском", async () => {
    const api = setup("en");
    api.pickedFiles = ["menu.pdf"];
    fireEvent.click(await screen.findByRole("button", { name: "Attach file" }));
    expect(await screen.findByRole("button", { name: "Remove menu.pdf" })).toBeInTheDocument();
    api.failNext("attachPick", "disk_full");
    fireEvent.click(screen.getByRole("button", { name: "Attach file" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("There is not enough disk space");
  });
});
