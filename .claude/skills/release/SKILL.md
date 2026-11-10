---
name: release
description: Cutting a Kinige release (semver per slice, scripts/release.sh, the tag, release.yml with the macOS dmg/zip and the Windows installer). Use only for the coordinator after the user accepted a slice or a batch of fixes, or when a release build failed.
---

# Releases (ADR-0012)

**Versions:** semver.
- The minor version is the slice number: 0.5.0 is slices 0–5.
- The patch version is for fixes between slices.
- 1.0.0 comes after sync.

**Where the version lives:** in `[workspace.package]` of `Cargo.toml`. `tauri.conf.json` has none of its own. The other files and `web/src/version.test.ts` repeat it and check it; don't change them by hand.

**Who cuts a release:** only the coordinator, after acceptance of a slice or a batch of fixes.

**Steps:**
1. `scripts/release.sh X.Y.Z` creates the branch `release/vX.Y.Z`, the commit and the PR. They go without a Huly number (CLAUDE.md rule 15).
2. Wait for CI, then merge.
3. `scripts/release.sh --tag X.Y.Z` puts the tag `vX.Y.Z` on `main`. It does so only when `ci.yml` is green on that commit.
4. The `release.yml` workflow builds the `.dmg` and `.zip` on a macOS runner and the Windows installer on `windows-2025`. It creates a GitHub Release with one `SHA256SUMS`.

**Progress:** `gh run list --workflow release.yml`, or wait with `gh run watch <id> --exit-status`.

**Rerun for a tag:** `gh workflow run release.yml -f tag=vX.Y.Z`. Add `-f publish=false` to build only.

**A tag is never moved or deleted.** A faulty release is fixed by the next patch.
