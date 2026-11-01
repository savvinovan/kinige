import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Версия приложения живёт в одном месте — `[workspace.package] version` в Cargo.toml (ADR-0012). Остальные файлы её
// повторяют (package.json, lock-файлы) или не имеют своей (tauri.conf.json: Tauri берёт версию из Cargo.toml).
// Тест ловит забытый файл, пока релиз не ушёл на сборку: `scripts/release.sh` меняет все файлы сразу.
const ROOT = join(import.meta.dirname, "..", "..");
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const WORKSPACE_CRATES = ["kinige", "kinige-domain", "kinige-mcp", "kinige-store"];

const read = (path: string) => readFileSync(join(ROOT, path), "utf8");

function workspaceVersion(): string {
  const section = /\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/.exec(read("Cargo.toml"))?.[1] ?? "";
  const match = /^version\s*=\s*"([^"]+)"/m.exec(section);
  if (!match?.[1]) throw new Error("в Cargo.toml нет [workspace.package] version");
  return match[1];
}

describe("версия приложения", () => {
  const version = workspaceVersion();

  it("в Cargo.toml — semver x.y.z без приставок", () => {
    expect(version).toMatch(SEMVER);
  });

  it("web/package.json и web/package-lock.json совпадают с Cargo.toml", () => {
    const pkg = JSON.parse(read("web/package.json")) as { version: string };
    const lock = JSON.parse(read("web/package-lock.json")) as {
      version: string;
      packages: Record<string, { version: string }>;
    };
    expect(pkg.version).toBe(version);
    expect(lock.version).toBe(version);
    expect(lock.packages[""]?.version).toBe(version);
  });

  it("Cargo.lock: у крейтов рабочей области та же версия", () => {
    const lock = read("Cargo.lock");
    for (const name of WORKSPACE_CRATES) {
      const entry = new RegExp(`\\[\\[package\\]\\]\\nname = "${name}"\\nversion = "([^"]+)"\\n`).exec(lock);
      expect(entry?.[1], `Cargo.lock: ${name}`).toBe(version);
    }
  });

  it("tauri.conf.json без своей версии: иначе их стало бы две", () => {
    const config = JSON.parse(read("src-tauri/tauri.conf.json")) as { version?: string };
    expect(config.version).toBeUndefined();
  });

  it("у крейтов рабочей области версия наследуется", () => {
    for (const path of ["src-tauri/Cargo.toml", "crates/kinige-domain/Cargo.toml", "crates/kinige-mcp/Cargo.toml", "crates/kinige-store/Cargo.toml"]) {
      expect(read(path), path).toMatch(/^version\.workspace = true$/m);
    }
  });
});
