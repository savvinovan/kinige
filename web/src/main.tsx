import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { EmptyWindow } from "./EmptyWindow";
import { TauriApi } from "./api/TauriApi";
import "./app.css";

const api = new TauriApi();
const root = document.getElementById("root");
if (!root) throw new Error("нет элемента #root");

createRoot(root).render(
  <StrictMode>
    <EmptyWindow api={api} />
  </StrictMode>,
);
