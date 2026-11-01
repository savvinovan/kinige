## Installation (macOS, Apple Silicon)

Download `Kinige-*-macos-arm64.dmg` (or the `.zip`) and drag **Kinige** into "Applications".

The build is signed ad-hoc, without a Developer ID and Apple notarization (this stays so until slice 10), so macOS stops it on first launch. Use any of these ways:

- macOS 15 and later: launch `Kinige.app` once, then "System Settings" -> "Privacy & Security" -> at the bottom, "Open Anyway" for Kinige;
- macOS 13-14: right-click `Kinige.app` -> "Open" -> "Open" (on macOS 15 and later this no longer bypasses the check);
- on any version, in Terminal: `xattr -dr com.apple.quarantine /Applications/Kinige.app`.

File check: `shasum -a 256 -c --ignore-missing SHA256SUMS` next to the downloaded files.

## Installation (Windows 10 and 11, x64)

Download `Kinige-*-windows-x64-setup.exe` and run it. The installer is per-user: it does not ask for administrator rights and puts Kinige into your user profile. If the Microsoft WebView2 runtime is missing (Windows 11 and updated Windows 10 already have it), the installer downloads it.

The installer is not signed (code signing comes in slice 10), so SmartScreen shows "Windows protected your PC". Click "More info" -> "Run anyway".

File check, in PowerShell next to the downloaded files: `Get-FileHash -Algorithm SHA256 .\Kinige-*-windows-x64-setup.exe` and compare the hash with the line in `SHA256SUMS`.

AI agent access (MCP) is not available on Windows yet: the "AI agent access" page in Settings says so. It comes with the next Windows step (slice 10).

## Data

- macOS: the database is in `~/Library/Application Support/me.savvinov.kinige/`, the Markdown copy of the entries in `~/Documents/Kinige`.
- Windows: the database is in `%APPDATA%\me.savvinov.kinige`, the Markdown copy of the entries in `Documents\Kinige` (your Documents folder).
