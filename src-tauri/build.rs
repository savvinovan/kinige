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
        embed_manifest_in_tests();
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
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("не удалось выполнить tauri-build");
}

/// Common Controls v6 для тестовых исполняемых файлов на Windows (слайс 10a). Диалог выбора папки (`rfd` через
/// `tauri-plugin-dialog`) вызывает `TaskDialogIndirect`, которой нет в comctl32 версии 5: без этой зависимости в манифесте
/// любой тест, слинкованный с приложением, не запускается (`STATUS_ENTRYPOINT_NOT_FOUND`). Для самого приложения манифест
/// кладёт `tauri-build`, но только в бинарники, поэтому для тестов он вставляется здесь.
fn embed_manifest_in_tests() {
    const COMMON_CONTROLS_V6: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#;
    let out_dir = std::env::var_os("OUT_DIR").expect("cargo не передал OUT_DIR");
    let manifest = std::path::Path::new(&out_dir).join("tests.manifest");
    std::fs::write(&manifest, COMMON_CONTROLS_V6).expect("не удалось записать манифест тестов");
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        manifest.display()
    );
}
