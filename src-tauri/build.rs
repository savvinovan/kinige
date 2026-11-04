// The build script reports an error only by panicking (that is how cargo stops it); it does not affect the app.
#![allow(clippy::expect_used)]

fn main() {
    // `generate_context!` fails if the `frontendDist` directory is missing. So that `cargo clippy`
    // and `cargo test` work on a clean clone without building the frontend, we create an empty directory;
    // the real content is put there by `npm run build` (`beforeBuildCommand`).
    std::fs::create_dir_all("../web/dist").expect("could not create web/dist");

    // `bundle.externalBin` requires the file `binaries/kinige-mcp-<triple>` for any build, including `cargo clippy` and
    // `cargo test` on a clean clone. An empty stub (it is in .gitignore) keeps the build from failing; the real bridge is put there
    // by `scripts/build-app.sh` and `scripts/run-app.sh`. The app treats an empty file as "bridge not found".
    // On Windows there is no bridge in the bundle (`tauri.windows.conf.json` empties `externalBin`, slice 10a): no stub needed.
    let target = std::env::var("TARGET").expect("cargo did not pass TARGET");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    if windows {
        embed_manifest();
    }
    let sidecar = std::path::Path::new("binaries").join(format!("kinige-mcp-{target}"));
    if !windows && !sidecar.exists() {
        std::fs::create_dir_all("binaries").expect("could not create src-tauri/binaries");
        std::fs::write(&sidecar, []).expect("could not create the bridge stub");
    }

    // Our own commands get `allow-<name>` permissions and are granted to nobody by default:
    // a window can call only what is listed in its capability. The list matches `kinige_lib::COMMANDS`
    // (the test `tests/config.rs` checks this).
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
        "set_chat_color",
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
        "get_forest_year",
        "get_forest_day_target",
        "set_forest_view",
        "attach_pick",
        "attach_paste",
        "attach_dropped",
        "remove_draft_attachment",
        "open_attachment",
        "send_voice",
        "open_microphone_settings",
        "read_voice_for_waveform",
        "set_attachment_waveform",
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
    // On Windows `embed_manifest` puts the manifest (Common Controls v6) into every executable at once, so `tauri-build`
    // does not add its own: the linker does not accept two manifests in one file.
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
    .expect("tauri-build failed");
    if windows {
        link_resources_into_tests();
    }
}

/// A manifest with Common Controls v6 for every executable of the crate on Windows (slice 10a): the app, the tests in
/// `tests/` and the library tests. The folder dialog (`rfd` through `tauri-plugin-dialog`) calls `TaskDialogIndirect`,
/// which comctl32 version 5 does not have: without this dependency in the manifest any executable linked with the app
/// does not start (`STATUS_ENTRYPOINT_NOT_FOUND`). The default manifest of `tauri-build` is the same, but it goes into
/// binaries only; so the linker embeds the manifest and `tauri-build` adds none (`new_without_app_manifest`).
fn embed_manifest() {
    const COMMON_CONTROLS_V6: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#;
    let out_dir = std::env::var_os("OUT_DIR").expect("cargo did not pass OUT_DIR");
    let manifest = std::path::Path::new(&out_dir).join("app.manifest");
    std::fs::write(&manifest, COMMON_CONTROLS_V6).expect("could not write the manifest");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}

/// The app resources (window icon, version info) for integration tests on Windows. `tauri-build` links them into binaries
/// only, while `generate_context!` takes the default window icon from the resource of the executable itself: in a test
/// without the resource the context cannot be created ("failed to load the default window icon"). The file
/// `resource.lib` is in `OUT_DIR` after `tauri-build`.
fn link_resources_into_tests() {
    let out_dir = std::env::var_os("OUT_DIR").expect("cargo did not pass OUT_DIR");
    let resources = std::path::Path::new(&out_dir).join("resource.lib");
    println!("cargo:rustc-link-arg-tests={}", resources.display());
}
