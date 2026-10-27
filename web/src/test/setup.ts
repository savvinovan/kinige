import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";

// Ожидания `findBy*` / `waitFor` по умолчанию — 1 с; на нагруженной машине (параллельная сборка Rust) тесты контейнера
// главного окна иногда не укладывались: запас не делает зелёные тесты медленнее.
configure({ asyncUtilTimeout: 4000 });

afterEach(cleanup);
