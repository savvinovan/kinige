// Скрипт сборки сообщает об ошибке только паникой (так его останавливает cargo); приложение он не затрагивает.
#![allow(clippy::expect_used)]

fn main() {
    // `generate_context!` падает, если нет каталога `frontendDist`. Чтобы `cargo clippy`
    // и `cargo test` работали на чистом клоне без сборки фронтенда, создаём пустой каталог;
    // настоящее содержимое кладёт `npm run build` (`beforeBuildCommand`).
    std::fs::create_dir_all("../web/dist").expect("не удалось создать web/dist");

    // `bundle.externalBin` требует файл `binaries/kinige-mcp-<triple>` при любой сборке, в том числе `cargo clippy` и
    // `cargo test` на чистом клоне. Пустая заглушка (она в .gitignore) не даёт сборке упасть; настоящий мост кладут туда
    // `scripts/build-app.sh` и `scripts/run-app.sh`. Приложение считает пустой файл «мост не найден».
    // На Windows моста в бандле нет (`tauri.windows.conf.json` опустошает `externalBin`, слайс 10a): заглушка не нужна.
    let target = std::env::var("TARGET").expect("cargo не передал TARGET");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    if windows {
        embed_manifest();
    }
    let sidecar = std::path::Path::new("binaries").join(format!("kinige-mcp-{target}"));
    if !windows && !sidecar.exists() {
        std::fs::create_dir_all("binaries").expect("не удалось создать src-tauri/binaries");
        std::fs::write(&sidecar, []).expect("не удалось создать заглушку моста");
    }

    // Собственные команды получают разрешения `allow-<имя>` и по умолчанию никому не выданы:
    // окно может вызвать только то, что перечислено в его capability. Список совпадает с `kinige_lib::COMMANDS`
    // (это проверяет тест `tests/config.rs`).
    let manifest = tauri_build::AppManifest::new().commands(&[
        "app_info",
        "get_language",
        "list_chats",
        "list_messages",
        "send_message",
        "get_settings",
        "open_external_url",
        "get_draft",
        "save_draft",
        "create_chat",
        "rename_chat",
        "set_chat_icon",
        "set_chat_pinned",
        "set_chat_archived",
        "delete_message",
        "restore_message",
        "edit_message",
        "toggle_task_item",
        "set_message_flag",
        "set_message_date",
        "move_message",
        "list_trash",
        "purge_message",
        "set_sidebar_collapsed",
        "add_recent_emoji",
        "startup_state",
        "restore_backup",
        "restart_for_recovery",
        "import_telegram_open",
        "import_telegram_run",
        "import_telegram_stop",
        "import_telegram_close",
        "search_messages",
        "open_settings",
        "get_on_this_day",
        "dismiss_on_this_day",
        "set_text_size",
        "choose_mirror_folder",
        "set_shortcut",
        "reset_shortcut",
        "shortcut_recording",
        "get_agent_access",
        "set_agent_enabled",
        "set_agent_mode",
        "set_agent_chats",
        "list_agent_log",
        "list_agent_log_entries",
        "get_settings_file",
        "settings_open_entry",
        "capture_state",
        "capture_send",
        "capture_append",
        "capture_save_draft",
        "capture_hide",
        "capture_resize",
        "capture_add_recent_emoji",
    ]);
    // На Windows манифест (Common Controls v6) кладёт `embed_manifest` для всех исполняемых файлов сразу, поэтому свой манифест
    // `tauri-build` не вставляет: два манифеста в одном файле линкер не принимает.
    let windows_attributes = if windows {
        tauri_build::WindowsAttributes::new_without_app_manifest()
    } else {
        tauri_build::WindowsAttributes::new()
    };
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(manifest)
            .windows_attributes(windows_attributes),
    )
    .expect("не удалось выполнить tauri-build");
    if windows {
        link_resources_into_tests();
    }
}

/// Манифест с Common Controls v6 для всех исполняемых файлов крейта на Windows (слайс 10a): приложения, тестов в `tests/` и
/// тестов библиотеки. Диалог выбора папки (`rfd` через `tauri-plugin-dialog`) вызывает `TaskDialogIndirect`, которой нет в
/// comctl32 версии 5: без этой зависимости в манифесте любой исполняемый файл, слинкованный с приложением, не запускается
/// (`STATUS_ENTRYPOINT_NOT_FOUND`). Манифест по умолчанию у `tauri-build` тот же, но он попадает только в бинарники; поэтому
/// манифест вставляет линкер, а `tauri-build` свой не добавляет (`new_without_app_manifest`).
fn embed_manifest() {
    const COMMON_CONTROLS_V6: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#;
    let out_dir = std::env::var_os("OUT_DIR").expect("cargo не передал OUT_DIR");
    let manifest = std::path::Path::new(&out_dir).join("app.manifest");
    std::fs::write(&manifest, COMMON_CONTROLS_V6).expect("не удалось записать манифест");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}

/// Ресурсы приложения (значок окна, сведения о версии) для интеграционных тестов на Windows. `tauri-build` линкует их только в
/// бинарники, а `generate_context!` берёт значок окна по умолчанию из ресурса самого исполняемого файла: в тесте без ресурса
/// контекст не создаётся ("failed to load the default window icon"). Файл `resource.lib` лежит в `OUT_DIR` после `tauri-build`.
fn link_resources_into_tests() {
    let out_dir = std::env::var_os("OUT_DIR").expect("cargo не передал OUT_DIR");
    let resources = std::path::Path::new(&out_dir).join("resource.lib");
    println!("cargo:rustc-link-arg-tests={}", resources.display());
}
