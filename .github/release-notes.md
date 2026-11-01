## Installation (macOS, Apple Silicon)

Download `Kinige-*-macos-arm64.dmg` (or the `.zip`) and drag **Kinige** into "Applications".

The build is signed ad-hoc, without a Developer ID and Apple notarization (this stays so until slice 10), so macOS stops it on first launch. Use any of these ways:

- macOS 15 and later: launch `Kinige.app` once, then "System Settings" -> "Privacy & Security" -> at the bottom, "Open Anyway" for Kinige;
- macOS 13-14: right-click `Kinige.app` -> "Open" -> "Open" (on macOS 15 and later this no longer bypasses the check);
- on any version, in Terminal: `xattr -dr com.apple.quarantine /Applications/Kinige.app`.

File check: `shasum -a 256 -c --ignore-missing SHA256SUMS` next to the downloaded files.

## Data

- Database: `~/Library/Application Support/me.savvinov.kinige/`.
- Markdown copy of the entries: `~/Documents/Kinige`.
