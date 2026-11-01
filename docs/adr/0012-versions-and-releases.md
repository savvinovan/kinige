# ADR-0012: Версии и релизы

Дата: 2026-11-01. Статус: **принято** (задача KINIG-54, не слайс). Решения пользователя: схема версий и порядок выпуска. Связано с ADR-0003 (подпись).

## Контекст
До этой задачи версия стояла 0.1.0 с каркаса, релизов не было: приложение собиралось только локально (`scripts/build-app.sh`). Нужны номера, по которым можно сказать «это сломалось в такой-то версии», и скачиваемая сборка.

## Решение

**Схема.** Семвер `x.y.z` без приставок. Пока приложение до 1.0: минор = номер слайса (после слайса 5 — `0.5.0`, после слайса 6 — `0.6.0`; подслайсы вроде 4a и 4b входят в минор своего слайса), патч — исправления между слайсами. `1.0.0` — после синхронизации (слайс 11). Мажор и минор после 1.0 — по смыслу изменений для пользователя, решает владелец.

**Один источник.** `[workspace.package] version` в `Cargo.toml`: крейты наследуют её (`version.workspace = true`), `tauri.conf.json` своей `version` не имеет и Tauri берёт версию из крейта (по документации `tauri-utils`: «если не задана, берётся из `Cargo.toml`»), поэтому `CFBundleShortVersionString` и `CFBundleVersion` равны ей; `scripts/build-app.sh` проверяет это по `Info.plist` собранного `.app`. `web/package.json` и lock-файлы повторяют версию; расхождение ловит `web/src/version.test.ts`. Версия видна в приложении: «Профиль» -> «О программе» (строка от Rust, `KinigeApi.appInfo()`).

**Кто и когда.** Релиз режет координатор, не автоматика на каждый мерж: после приёмки слайса и после пачки исправлений. Два явных шага, ничего на `main` само:
1. `scripts/release.sh X.Y.Z` — проверяет формат и что версия больше текущей, во временном worktree от `origin/main` поднимает версию во всех четырёх файлах (`Cargo.lock` только у крейтов рабочей области: `cargo update -w --offline`), коммит `Release vX.Y.Z` в ветке `release/vX.Y.Z`, push и PR. PR идёт обычным CI и мержится как все.
2. После мержа `scripts/release.sh --tag X.Y.Z` проверяет, что в `origin/main` именно эта версия и что CI (`ci.yml`) на этом коммите завершился успешно, и ставит аннотированный тег `vX.Y.Z` на `origin/main`.

**Что в релизе.** Тег запускает `.github/workflows/release.yml` (macOS-раннер `macos-15`, arm64): проверяет «тег = `v` + версия из `Cargo.toml`» и «коммит тега в `main`», собирает как `scripts/build-app.sh` (с мостом MCP), делает `.dmg` и `.zip`, проверяет подпись у приложения внутри образа и в распакованном архиве, считает `SHA256SUMS`. Отдельная короткая задача с правом записи создаёт GitHub Release: заметки = шаблон `.github/release-notes.md` (как открыть сборку с ad-hoc подписью, где лежат данные) + сгенерированный GitHub список изменений. Сборка идёт без кэшей, права на запись у задачи сборки нет. Повтор по готовому тегу — `workflow_dispatch` с вводом тега.

**Подпись.** До слайса 10 сборка подписана ad-hoc (ADR-0003): macOS при первом запуске остановит её, в заметках релиза описаны способы открыть (на macOS 15 и новее правый клик -> «Открыть» проверку больше не обходит: нужно «Всё равно открыть» в настройках конфиденциальности или `xattr`). Developer ID, нотаризация и updater — слайс 10; секреты подписи в workflow появятся тогда и только в задаче сборки релиза.

## Последствия
- Тег неизменяем по договорённости: ошибочный релиз не переписывается, выходит следующий патч.
- Intel-сборки нет (раннер arm64); universal-сборка — отдельное решение, если понадобится.
- Сгенерированные GitHub заметки перечисляют PR, а не коммиты; у первого релиза (0.5.0) это все PR с начала истории, список можно вычистить вручную.
- Исключение из правила 15 CLAUDE.md: релизный коммит и PR `Release vX.Y.Z` создаёт скрипт и идёт без номера задачи Huly (номер KINIG-54 к ним не подходит: релизов будет много, а задача одна).

## Windows artifacts (slice 10a, KINIG-61)

Decided by the user on 1.11.2026: the Windows build comes before the rest of slice 10, without signing and without MCP.

- **Runner and target.** `release.yml` has a second build job on `windows-2025` (x64, `x86_64-pc-windows-msvc`); `build` was renamed `build-macos`. Both jobs run the same checks of the tag (`vX.Y.Z`, equal to the version in `Cargo.toml`, the commit is in `main`) and build from scratch without caches.
- **Format.** An unsigned NSIS installer (`tauri build --bundles nsis`, `scripts/build-windows.sh`), per-user (`installMode: currentUser`): no administrator rights, installed under `%LOCALAPPDATA%`. WebView2 is downloaded by the installer if the machine lacks it (`downloadBootstrapper`, the default; an offline installer would add ~127 MB). MSI was not chosen: it needs elevation by default and gives nothing for a single-user app.
- **Name.** `Kinige-X.Y.Z-windows-x64-setup.exe`. The script checks that the installer name and `ProductVersion` in `kinige.exe` carry the version from `Cargo.toml` (the same single source as on macOS: `tauri-build` writes it into the exe resources).
- **No bridge.** `src-tauri/tauri.windows.conf.json` empties `bundle.externalBin`, so the MCP bridge is neither built nor bundled; `build.rs` makes no sidecar stub on Windows. The bridge crate still compiles there and exits with code 2 and "unsupported". The simpler option (build the bridge and ship a binary that only says "unsupported") would put a useless executable next to the app and invite a connection snippet that cannot work.
- **Checksums.** Each build job writes its own `SHA256SUMS.<os>`; `publish` merges them into the single `SHA256SUMS`, runs `sha256sum -c` over every file and only then creates the Release with the `.dmg`, `.zip` and `.exe`. The release notes (`.github/release-notes.md`) have a Windows section: SmartScreen ("More info" -> "Run anyway"), data locations (`%APPDATA%\me.savvinov.kinige`, `Documents\Kinige`), AI-agent access not available yet.
- **Dry run.** A push to a branch (when the workflow file lists it) runs both build jobs without the tag checks and without `publish`; this is how the workflow was checked before the first tag.

Consequences:
- SmartScreen warns on every download until Authenticode signing (slice 10); reputation builds only after signing.
- No auto-update on Windows until the updater (slice 10); updates are manual, the installer upgrades in place.
- ARM64 Windows is not built (x64 runs there under emulation).
