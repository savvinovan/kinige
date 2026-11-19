//! Slice 7b outside the window: a hidden photo is an ordinary photo in search, the Markdown copy and MCP; "Open in default app"
//! decides from the original name; migration 9 leaves every older row as it was.

#![cfg(test)]

mod common;

use common::agent::{agent, call, entry_ids};
use common::attach::{jpeg, write};
use common::days::at;
use common::details::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::Now;
use serde_json::json;

fn now() -> Now {
    at(2026, 11, 18, 12, 0)
}

fn later() -> Now {
    at(2026, 11, 18, 12, 5)
}

fn hits(store: &kinige_store::Store, query: &str) -> Vec<String> {
    store
        .search(query, &Default::default(), None, 20)
        .unwrap()
        .hits
        .into_iter()
        .map(|hit| hit.message.id)
        .collect()
}

#[test]
fn search_finds_an_attachment_by_its_new_name_and_description_hidden_or_not_and_not_by_the_old_name()
 {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = photo_and_pdf(&store, dir.path(), &now());
    let (photo, pdf) = (
        sent.attachments[0].id.clone(),
        sent.attachments[1].id.clone(),
    );
    store
        .send_message(INBOX_ID, "совсем другое", &now())
        .unwrap();
    assert_eq!(
        hits(&store, "IMG_0001"),
        vec![sent.id.clone()],
        "the original name, before a rename"
    );

    store
        .set_attachment_details(
            &photo,
            &changes(json!({"name": "beach", "description": "Sunset, July", "spoiler": true})),
            &later(),
        )
        .unwrap();
    for query in ["beach", "sunset", "july"] {
        assert_eq!(
            hits(&store, query),
            vec![sent.id.clone()],
            "{query}: found although the photo is hidden"
        );
    }
    assert!(
        hits(&store, "IMG_0001").is_empty(),
        "a renamed photo is found by its new name only"
    );
    assert!(
        hits(&store, "jpg").is_empty(),
        "the extension is not indexed"
    );
    // A hit shows the entry's own text, never a picture.
    let found = store
        .search("beach", &Default::default(), None, 20)
        .unwrap();
    assert_eq!(found.hits[0].message.attachments[0].name, "beach.jpg");

    // Showing the photo again changes nothing in the index; clearing the description takes it out.
    store
        .set_attachment_details(&photo, &changes(json!({"spoiler": false})), &later())
        .unwrap();
    assert_eq!(hits(&store, "sunset"), vec![sent.id.clone()]);
    store
        .set_attachment_details(&photo, &changes(json!({"description": null})), &later())
        .unwrap();
    assert!(hits(&store, "sunset").is_empty());
    store
        .set_attachment_details(
            &pdf,
            &changes(json!({"name": "обед", "description": "меню на неделю"})),
            &later(),
        )
        .unwrap();
    assert_eq!(hits(&store, "меню"), vec![sent.id.clone()]);
    assert!(
        hits(&store, "menu").is_empty(),
        "the PDF's old name is gone from the index"
    );
}

#[test]
fn the_markdown_copy_writes_a_hidden_photo_like_any_other_and_a_description_cannot_break_the_day_file()
 {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let sent = photo_and_pdf(&store, dir.path(), &now());
    let (photo, pdf) = (
        sent.attachments[0].id.clone(),
        sent.attachments[1].id.clone(),
    );
    let day = mirror_dir(dir.path()).join("Inbox/2026");
    let (day_file, folder) = (day.join("2026-11-18.md"), day.join("2026-11-18"));
    wait_until("the first copy", || {
        std::fs::read_to_string(&day_file).is_ok_and(|text| text.contains("menu.pdf"))
    });

    store
        .set_attachment_details(
            &photo,
            &changes(json!({"name": "beach", "description": "Beach, July", "spoiler": true})),
            &later(),
        )
        .unwrap();
    wait_until("the renamed photo in the copy", || {
        std::fs::read_to_string(&day_file).is_ok_and(|text| text.contains("beach.jpg"))
    });
    let text = std::fs::read_to_string(&day_file).unwrap();
    assert!(
        text.contains("![Beach, July](<2026-11-18/beach.jpg>)"),
        "{text}"
    );
    assert!(text.contains("[menu.pdf](<2026-11-18/menu.pdf>)"), "{text}");
    wait_until("the files", || {
        common::attach::names(&folder) == ["beach.jpg", "menu.pdf"]
    });

    // The file card: a new name, and the description is the link text. Everything that could break the file is escaped or flattened.
    let hostile = "line one\n# not a heading\n- not a list\n`code` <b>bold</b> [x](y) \\ end";
    store
        .set_attachment_details(
            &pdf,
            &changes(json!({"name": "обед", "description": hostile})),
            &later(),
        )
        .unwrap();
    // The file is copied before the day file is rewritten: wait for both, or the test reads the old day file.
    wait_until(
        "the PDF under its new name, in the folder and in the day file",
        || {
            common::attach::names(&folder).contains(&"обед.pdf".to_string())
                && std::fs::read_to_string(&day_file).is_ok_and(|text| text.contains("обед.pdf"))
        },
    );
    let text = std::fs::read_to_string(&day_file).unwrap();
    assert!(
        text.contains(
            "[line one # not a heading - not a list \\`code\\` \\<b\\>bold\\</b\\> \\[x\\](y) \\\\ end](<2026-11-18/обед.pdf>)"
        ),
        "{text}"
    );
    let structure: Vec<&str> = text.lines().filter(|line| line.starts_with('#')).collect();
    assert_eq!(
        structure,
        ["# 2026-11-18", "## 12:00"],
        "no heading came out of a description"
    );
    assert_eq!(
        text.lines()
            .filter(|line| line.contains("обед.pdf"))
            .count(),
        1,
        "the link is one line"
    );
    assert!(!text.contains("<b>"), "raw HTML is escaped");

    // A second rename removes the file the copy wrote under the old name, and only that one.
    std::fs::write(folder.join("mine.txt"), b"mine").unwrap();
    store
        .set_attachment_details(&photo, &changes(json!({"name": "sea"})), &later())
        .unwrap();
    wait_until(
        "the old file to go and the day file to name the new one",
        || {
            common::attach::names(&folder) == ["mine.txt", "sea.jpg", "обед.pdf"]
                && std::fs::read_to_string(&day_file)
                    .is_ok_and(|text| text.contains("![Beach, July](<2026-11-18/sea.jpg>)"))
        },
    );
}

#[test]
fn an_agent_sees_the_name_the_description_and_the_flag_of_a_hidden_photo() {
    let a = agent(AgentMode::Read, true);
    let source = a.dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let when = moscow(T0);
    let photo = a
        .store
        .attach_path(
            &a.work,
            &write(&source, "IMG_0001.jpg", &jpeg(24, 18)),
            &when,
        )
        .unwrap();
    let pdf = a
        .store
        .attach_path(&a.work, &write(&source, "menu.pdf", b"%PDF"), &when)
        .unwrap();
    let sent = a
        .store
        .send_entry(
            &a.work,
            "обед",
            None,
            &[photo.id.clone(), pdf.id.clone()],
            &when,
        )
        .unwrap();
    a.store
        .set_attachment_details(
            &photo.id,
            &changes(json!({"name": "beach", "description": "Beach, July", "spoiler": true})),
            &when,
        )
        .unwrap();
    a.store
        .set_attachment_details(
            &pdf.id,
            &changes(json!({"name": "обед", "description": "меню"})),
            &when,
        )
        .unwrap();

    let reply = call(&a.store, "get_entry", json!({"id": sent.id}), T0).unwrap();
    let attachments = &reply["entry"]["attachments"];
    assert_eq!(
        attachments[0],
        json!({"kind": "image", "name": "beach.jpg", "description": "Beach, July", "spoiler": true,
               "bytes": sent.attachments[0].byte_size, "width": 24, "height": 18})
    );
    // A file card has no flag at all, and a photo that is not hidden has none either.
    assert_eq!(attachments[1]["name"], "обед.pdf");
    assert_eq!(attachments[1]["description"], "меню");
    assert!(attachments[1].get("spoiler").is_none());
    a.store
        .set_attachment_details(
            &photo.id,
            &changes(json!({"spoiler": false, "description": null})),
            &when,
        )
        .unwrap();
    let again = call(&a.store, "get_entry", json!({"id": sent.id}), T0).unwrap();
    assert!(
        again["entry"]["attachments"][0].get("spoiler").is_none(),
        "{again}"
    );
    assert!(
        again["entry"]["attachments"][0]
            .get("description")
            .is_none(),
        "{again}"
    );
    // Search sees the new name, and the reply never carries bytes or the stored file's name.
    let found = call(&a.store, "search", json!({"query": "beach"}), T0).unwrap();
    assert_eq!(entry_ids(&found), vec![sent.id.clone()]);
    assert!(!again.to_string().contains(&format!("{}.jpg", photo.id)));
}

#[test]
fn open_in_default_app_decides_from_the_original_name_and_a_rename_gives_at_most_the_stem() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).unwrap();
    let doc = store
        .attach_path(INBOX_ID, &write(&source, "notes.pdf", b"%PDF-1.4"), &now())
        .unwrap();
    let script = store
        .attach_path(INBOX_ID, &write(&source, "run.sh", b"echo hi"), &now())
        .unwrap();
    let readme = store
        .attach_path(INBOX_ID, &write(&source, "README", b"read me"), &now())
        .unwrap();
    let sent = store
        .send_entry(
            INBOX_ID,
            "",
            None,
            &[doc.id.clone(), script.id.clone(), readme.id.clone()],
            &now(),
        )
        .unwrap();
    let before: Vec<bool> = sent
        .attachments
        .iter()
        .map(|a| store.prepare_open(&a.id).unwrap().reveal_only)
        .collect();
    assert_eq!(before, [false, true, true]);

    for (id, stem) in [
        (&doc.id, "report"),
        (&script.id, "deploy"),
        (&readme.id, "guide"),
    ] {
        store
            .set_attachment_details(id, &changes(json!({ "name": stem })), &later())
            .unwrap();
    }
    let after: Vec<_> = sent
        .attachments
        .iter()
        .map(|a| store.prepare_open(&a.id).unwrap())
        .collect();
    assert_eq!(
        after.iter().map(|t| t.reveal_only).collect::<Vec<_>>(),
        before,
        "a rename changes no verdict"
    );
    let clone_names: Vec<String> = after
        .iter()
        .map(|t| t.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        clone_names,
        ["report.pdf", "deploy.sh", "guide.bin"],
        "the stem is the display name's, the extension the original's"
    );
    // A name that would give `README` a document extension is refused before it can matter.
    let error = store
        .set_attachment_details(
            &readme.id,
            &changes(json!({"name": "README.doc"})),
            &later(),
        )
        .unwrap_err();
    assert_eq!(
        error.code(),
        kinige_domain::ErrorCode::AttachmentDetailsInvalid
    );
}

#[test]
fn migration_nine_leaves_every_older_row_byte_identical_and_shows_its_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let (photo, entry) = {
        let store = open_without_mirror(dir.path());
        let sent = photo_and_pdf(&store, dir.path(), &now());
        let photo = sent.attachments[0].id.clone();
        make_legacy(dir.path(), &photo);
        (photo, sent.id)
    };
    let before = (groups(dir.path(), &photo), hlcs(dir.path(), &photo));
    assert_eq!(before.0.len(), 3, "a version-8 row has no details groups");
    common::old_db::downgrade_to(dir.path(), 8);
    let has_column = |name: &str| {
        raw(dir.path())
            .prepare("SELECT name FROM pragma_table_info('attachment')")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .flatten()
            .any(|column| column == name)
    };
    assert!(
        !has_column("spoiler"),
        "the downgraded database is a version-8 one"
    );

    let store = open_without_mirror(dir.path());
    assert!(has_column("spoiler") && has_column("display_name") && has_column("description"));
    assert_eq!(
        (groups(dir.path(), &photo), hlcs(dir.path(), &photo)),
        before,
        "no backfill, no repair"
    );
    let shown = view_in_feed(&store, &entry, &photo, &later());
    assert_eq!(
        (shown.name.as_str(), shown.description, shown.spoiler),
        ("IMG_0001.jpg", None, false)
    );
    // The draft table got its columns too.
    let tile = store
        .attach_path(
            INBOX_ID,
            &write(&dir.path().join("src"), "b.jpg", &jpeg(8, 8)),
            &later(),
        )
        .unwrap();
    assert_eq!((tile.name.as_str(), tile.spoiler), ("b.jpg", false));
}
