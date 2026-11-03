import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";

// The `findBy*` / `waitFor` waits default to 1 s; on a loaded machine (a parallel Rust build) the main window container
// tests sometimes did not fit: the margin does not make green tests slower.
configure({ asyncUtilTimeout: 4000 });

afterEach(() => {
  cleanup();
  // The settings tree state and the like must not pass from one test to another.
  try {
    localStorage.clear();
  } catch {
    // no storage - nothing to clear
  }
});
