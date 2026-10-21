import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { EmptyWindow } from "./EmptyWindow";
import type { KinigeApi } from "./api/KinigeApi";
import { MockApi } from "./api/MockApi";

describe("EmptyWindow", () => {
  it("рисует строку версии из KinigeApi", async () => {
    render(<EmptyWindow api={new MockApi()} />);
    expect(await screen.findByText("Kinige mock")).toBeInTheDocument();
  });

  it("показывает ошибку бэкенда, а не пустое окно", async () => {
    const failing = { appInfo: () => Promise.reject(new Error("нет связи")) } as unknown as KinigeApi;
    render(<EmptyWindow api={failing} />);
    expect(await screen.findByText(/нет связи/)).toBeInTheDocument();
  });
});
