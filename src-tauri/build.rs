fn main() {
    // `generate_context!` падает, если нет каталога `frontendDist`. Чтобы `cargo clippy`
    // и `cargo test` работали на чистом клоне без сборки фронтенда, создаём пустой каталог;
    // настоящее содержимое кладёт `npm run build` (`beforeBuildCommand`).
    std::fs::create_dir_all("../web/dist").expect("не удалось создать web/dist");

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
        "set_text_size",
        "choose_mirror_folder",
        "open_external_url",
        "get_draft",
        "save_draft",
        "set_shortcut",
        "reset_shortcut",
        "shortcut_recording",
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
