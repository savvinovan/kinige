import { describe, expect, it } from "vitest";
import { render, screen } from "../../test/render";
import { TooNewScreen } from "./TooNewScreen";

describe("TooNewScreen", () => {
  it("главная область с заголовком и пояснением, без кнопок и полей", () => {
    render(<TooNewScreen />);
    expect(screen.getByRole("main", { name: "База создана более новой версией Kinige" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "База создана более новой версией Kinige" })).toBeInTheDocument();
    expect(screen.getByText("Откат версии не поддерживается: обновите приложение. Ничего не изменено и не удалено.")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("radio")).toBeNull();
  });

  it("английский", () => {
    render(<TooNewScreen />, { language: "en" });
    expect(screen.getByRole("heading", { name: "The database was created by a newer version of Kinige" })).toBeInTheDocument();
    expect(screen.getByText("Downgrading is not supported: update the app. Nothing has been changed or deleted.")).toBeInTheDocument();
  });
});
