use super::*;
use crate::groups::parse_groups_raw;

const ID: &str = "9d3e4f50-6a7b-4c8d-9e0f-1a2b3c4d5e6f";
const MESSAGE: &str = "5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d";
const BLOB: &str = "01934f2e-8a7b-7c3d-9e1f-2a3b4c5d6e7f";
const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// The bytes of appendix §C.6 (ADR-0001): changing them changes the sync format.
const CORE_REFERENCE: &str = r#"{"blob_key":"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f","byte_size":245760,"duration_ms":null,"filename":"IMG_0001.jpg","height":960,"kind":"image","link_site":null,"link_title":null,"message_id":"5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d","mime":"image/jpeg","remote_blob_id":"01934f2e-8a7b-7c3d-9e1f-2a3b4c5d6e7f","sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","url":null,"width":1280}"#;

fn input() -> NewAttachment<'static> {
    NewAttachment {
        id: ID,
        message_id: MESSAGE,
        kind: AttachmentKind::Image,
        sort_index: 0,
        sha256: SHA,
        filename: "IMG_0001.jpg",
        mime: "image/jpeg",
        byte_size: 245_760,
        width: Some(1280),
        height: Some(960),
        duration_ms: None,
        remote_blob_id: BLOB,
        blob_key: KEY,
    }
}

fn hlc() -> String {
    format!("{:013}-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f", 1_793_000_000_000u64)
}

#[test]
fn the_core_group_matches_the_reference_bytes_of_appendix_c6() {
    let row = new_attachment(&input(), &hlc()).unwrap();
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    assert_eq!(groups["core"].get(), CORE_REFERENCE);
    assert_eq!(groups["order"].get(), r#"{"sort_index":0}"#);
    assert_eq!(groups["deleted"].get(), r#"{"deleted_at":null}"#);
    assert_eq!(groups.len(), 3);
    let hlcs: serde_json::Value = serde_json::from_str(&row.hlc_json).unwrap();
    for group in ["core", "order", "deleted"] {
        assert_eq!(hlcs[group]["h"], hlc().as_str());
        assert_eq!(hlcs[group]["sv"], 1);
    }
    assert_eq!(row.schema_version, 1);
}

#[test]
fn a_file_has_null_optionals_and_no_dimensions() {
    let mut file = input();
    file.kind = AttachmentKind::File;
    file.width = None;
    file.height = None;
    file.filename = "menu.pdf";
    file.mime = "application/pdf";
    let row = new_attachment(&file, &hlc()).unwrap();
    let core = parse_groups_raw(&row.groups_raw).unwrap()["core"]
        .get()
        .to_string();
    assert!(core.contains(r#""height":null"#), "{core}");
    assert!(core.contains(r#""width":null"#), "{core}");
    assert!(core.contains(r#""kind":"file""#), "{core}");
    // A file with dimensions is a mistake of the caller.
    file.width = Some(1);
    assert!(new_attachment(&file, &hlc()).is_err());
}

#[test]
fn values_outside_the_format_never_reach_the_bytes() {
    let long_name = "a".repeat(256);
    let not_hex = "g".repeat(64);
    let upper_key = KEY.to_uppercase();
    let short_sha = &SHA[1..];
    let bad: Vec<NewAttachment<'_>> = vec![
        NewAttachment { id: "x", ..input() },
        NewAttachment { message_id: "", ..input() },
        // Upper case, wrong version, wrong variant.
        NewAttachment { remote_blob_id: "01934F2E-8A7B-7C3D-9E1F-2A3B4C5D6E7F", ..input() },
        NewAttachment { remote_blob_id: MESSAGE, ..input() },
        NewAttachment { remote_blob_id: "01934f2e-8a7b-7c3d-7e1f-2a3b4c5d6e7f", ..input() },
        NewAttachment { sha256: short_sha, ..input() },
        NewAttachment { blob_key: &not_hex, ..input() },
        NewAttachment { blob_key: &upper_key, ..input() },
        NewAttachment { filename: "", ..input() },
        NewAttachment { filename: "a\u{0}b.jpg", ..input() },
        NewAttachment { filename: "dir/a.jpg", ..input() },
        NewAttachment { filename: "evil\u{202E}gpj.exe", ..input() },
        NewAttachment { filename: long_name.as_str(), ..input() },
        NewAttachment { mime: "", ..input() },
        NewAttachment { mime: "image/jpeg\n", ..input() },
        NewAttachment { byte_size: -1, ..input() },
        NewAttachment { byte_size: (MAX_FILE_BYTES as i64) + 1, ..input() },
        NewAttachment { sort_index: 10, ..input() },
    ];
    for (n, case) in bad.iter().enumerate() {
        assert!(new_attachment(case, &hlc()).is_err(), "case {n}");
    }
}

#[test]
fn the_largest_file_and_the_longest_valid_name_are_accepted() {
    let name = "я".repeat(127);
    assert_eq!(name.len(), 254);
    let mut edge = input();
    edge.byte_size = MAX_FILE_BYTES as i64;
    edge.filename = &name;
    edge.sort_index = 9;
    assert!(new_attachment(&edge, &hlc()).is_ok());
}

#[test]
fn hex_and_uuid_checks() {
    assert_eq!(hex_lower(&[0x00, 0xab, 0xff, 0x10]), "00abff10");
    assert_eq!(hex_lower(&[]), "");
    assert!(is_hex64(SHA));
    assert!(!is_hex64(&SHA.to_uppercase()));
    assert!(is_uuid_v7(BLOB));
    assert!(!is_uuid_v7(ID));
    assert!(!is_uuid_v7(""));
    assert!(!is_uuid_v7(&BLOB.replace('-', "")));
}

#[test]
fn filenames_are_cleaned_and_cut() {
    assert_eq!(sanitize_filename("IMG_0001.jpg"), "IMG_0001.jpg");
    assert_eq!(sanitize_filename("/Users/me/Pictures/a.png"), "a.png");
    assert_eq!(sanitize_filename(r"C:\Users\me\a.png"), "a.png");
    assert_eq!(sanitize_filename("a\u{0}\u{7}b\n.txt"), "ab.txt");
    assert_eq!(sanitize_filename("report\u{202E}fdp.exe"), "reportfdp.exe");
    assert_eq!(sanitize_filename("  spaced name.pdf  "), "spaced name.pdf");
    for empty in ["", " ", "/", "..", ".", "\u{0}", "a/", "\n"] {
        let cleaned = sanitize_filename(empty);
        assert!(cleaned == "file" || cleaned == "a", "{empty:?} -> {cleaned:?}");
    }
    let long = format!("{}.jpeg", "я".repeat(300));
    let cut = sanitize_filename(&long);
    assert!(cut.len() <= MAX_FILENAME_BYTES, "{}", cut.len());
    assert!(cut.ends_with(".jpeg"));
    // A fixed point: a stored name passes the check again.
    assert_eq!(sanitize_filename(&cut), cut);
    let spaced = format!("{} b.txt", "a".repeat(246));
    let cut = sanitize_filename(&format!("{spaced}{}", "x".repeat(300)));
    assert_eq!(sanitize_filename(&cut), cut);
}

#[test]
fn the_stored_extension_follows_the_regex_then_the_format_then_bin() {
    assert_eq!(stored_ext("IMG_0001.JPG", None), "jpg");
    assert_eq!(stored_ext("archive.tar.gz", None), "gz");
    assert_eq!(stored_ext("noext", None), "bin");
    assert_eq!(stored_ext("noext", Some(ImageFormat::WebP)), "webp");
    assert_eq!(stored_ext(".hidden", None), "bin");
    assert_eq!(stored_ext("a.", Some(ImageFormat::Png)), "png");
    assert_eq!(stored_ext("a.waytoolongext1", Some(ImageFormat::Gif)), "gif");
    assert_eq!(stored_ext("a.j p", Some(ImageFormat::Jpeg)), "jpg");
    assert_eq!(stored_ext("a.ex/../e", None), "bin");
    assert_eq!(stored_ext("photo.я", Some(ImageFormat::Jpeg)), "jpg");
    assert_eq!(stored_ext("a.0123456789", None), "0123456789");
}

#[test]
fn the_kind_comes_from_content_then_extension_or_mime() {
    let kind = |ext, mime, image| classify(ext, mime, image).0;
    assert_eq!(kind("jpg", None, Some(ImageFormat::Jpeg)), AttachmentKind::Image);
    // An image whose header failed is a file, whatever the extension says.
    assert_eq!(kind("jpg", None, None), AttachmentKind::File);
    assert_eq!(kind("heic", None, None), AttachmentKind::File);
    assert_eq!(kind("svg", None, None), AttachmentKind::File);
    assert_eq!(kind("mp4", None, None), AttachmentKind::Video);
    assert_eq!(kind("bin", Some("video/mp4"), None), AttachmentKind::Video);
    assert_eq!(kind("m4a", None, None), AttachmentKind::Audio);
    assert_eq!(kind("bin", Some("audio/ogg"), None), AttachmentKind::Audio);
    assert_eq!(kind("pdf", None, None), AttachmentKind::File);
    assert_eq!(kind("bin", Some("not a mime"), None), AttachmentKind::File);
    assert_eq!(classify("pdf", None, None).1, "application/pdf");
    assert_eq!(classify("bin", None, None).1, "application/octet-stream");
    assert_eq!(classify("bin", Some("application/x-custom"), None).1, "application/x-custom");
    assert_eq!(classify("bin", Some("a/b\nc"), None).1, "application/octet-stream");
    assert_eq!(classify("png", None, Some(ImageFormat::Png)).1, "image/png");
}

#[test]
fn only_document_types_are_opened_in_a_system_app() {
    for ext in ["pdf", "PNG", "docx", "mp4"] {
        assert!(is_openable_document(ext), "{ext}");
    }
    for ext in [
        "app", "command", "sh", "exe", "bat", "dmg", "pkg", "zip", "svg", "docm", "xlsm", "html",
        "jar", "scpt", "webloc", "bin", "", "terminal", "workflow",
    ] {
        assert!(!is_openable_document(ext), "{ext}");
    }
}

fn view(kind: AttachmentKind, name: &str) -> AttachmentView {
    AttachmentView {
        id: ID.to_string(),
        kind,
        filename: name.to_string(),
        mime: "x/y".to_string(),
        byte_size: 1,
        width: None,
        height: None,
        duration_ms: None,
        state: AttachmentState::Local,
    }
}

#[test]
fn the_summary_counts_photos_and_files() {
    assert_eq!(summary_of(&[]), AttachmentSummary::default());
    let one = summary_of(&[view(AttachmentKind::Image, "a.jpg")]);
    assert_eq!((one.photos, one.files, one.name.as_deref()), (1, 0, Some("a.jpg")));
    let mixed = summary_of(&[
        view(AttachmentKind::Image, "a.jpg"),
        view(AttachmentKind::Image, "b.jpg"),
        view(AttachmentKind::File, "menu.pdf"),
        view(AttachmentKind::Audio, "v.ogg"),
    ]);
    assert_eq!((mixed.photos, mixed.files, mixed.name), (2, 2, None));
}

#[test]
fn mirror_names_are_safe_and_unique_without_case() {
    let names = mirror_file_names(&["IMG_0001.jpg", "img_0001.JPG", "IMG_0001.jpg", "menu.pdf"]);
    assert_eq!(
        names,
        ["IMG_0001.jpg", "img_0001 (2).JPG", "IMG_0001 (3).jpg", "menu.pdf"]
    );
    let odd = mirror_file_names(&["a:b*c?.txt", "CON.txt", "nul", "dots...", "x ", ""]);
    assert_eq!(odd, ["a_b_c_.txt", "_CON.txt", "_nul", "dots", "x", "file"]);
    for name in mirror_file_names(&["../../etc/passwd", r"..\..\x.txt", &"я".repeat(400)]) {
        assert!(!name.contains(['/', '\\']), "{name}");
        assert!(name.len() <= 255);
    }
    // A suffix does not take the name past the limit.
    let long = "b".repeat(300);
    let pair = mirror_file_names(&[long.as_str(), long.as_str()]);
    assert!(pair.iter().all(|n| n.len() <= 255));
    assert_ne!(pair[0], pair[1]);
}
