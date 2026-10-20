import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MockApi } from "./api/MockApi";
import "./app.css";
import { MainWindow } from "./ui/screens/MainWindow";

// ВРЕМЕННО (слайс D1): главное окно на данных MockApi, чтобы увидеть дизайн в настоящем окне Tauri.
// Слайс 1 заменяет его настоящим экраном на TauriApi (команды Rust); EmptyWindow остался как каркас слайса 0.
const root = document.getElementById("root");
if (!root) throw new Error("нет элемента #root");

createRoot(root).render(
  <StrictMode>
    <MainWindow api={new MockApi()} />
  </StrictMode>,
);
