import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// The app version lives in one place - `[workspace.package] version` in Cargo.toml (ADR-0012). The other files repeat
// it (package.json, lock files) or have none of their own (tauri.conf.json: Tauri takes the version from Cargo.toml).
// The test catches a forgotten file before the release goes to build: `scripts/release.sh` changes all files at once.
const ROOT = join(import.meta.dirname, "..", "..");
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const WORKSPACE_CRATES = ["kinige", "kinige-domain", "kinige-mcp", "kinige-store"];

const read = (path: string) => readFileSync(join(ROOT, path), "utf8");

function workspaceVersion(): string {
  const section = /\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/.exec(read("Cargo.toml"))?.[1] ?? "";
  const match = /^version\s*=\s*"([^"]+)"/m.exec(section);
  if (!match?.[1]) throw new Error("no [workspace.package] version in Cargo.toml");
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
