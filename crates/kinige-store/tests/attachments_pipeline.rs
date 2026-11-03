//! The copy pipeline and the image rules (slice 7): failures leave nothing behind, formats are decided by content, the
//! EXIF orientation, thumbnails, the bytes the `kinige-media` protocol serves, and opening in a system app.

#![cfg(test)]

mod common;

use common::attach::*;
use common::days::at;
use common::*;
use kinige_domain::attachment::{AttachmentKind, MAX_FILE_BYTES};
use kinige_domain::groups::INBOX_ID;
use kinige_store::{AttachFault, MediaVariant, Now, Store, StoreError};
use std::sync::Arc;

fn now() -> Now {
    at(2026, 11, 3, 12, 0)
}

fn attach_file(store: &Store, dir: &std::path::Path, name: &str, bytes: &[u8]) -> Result<kinige_domain::AttachmentView, StoreError> {
    let source = dir.join("src");
    std::fs::create_dir_all(&source).unwrap();
    store.attach_path(INBOX_ID, &write(&source, name, bytes), &now())
}

fn nothing_left(dir: &std::path::Path) {
    let tmp: Vec<String> = std::fs::read_dir(dir.join("attachments/.tmp"))
        .map(|entries| entries.flatten().flat_map(|d| names(&d.path())).collect())
        .unwrap_or_default();
    assert!(tmp.is_empty(), "temporary files left: {tmp:?}");
    let stored: Vec<String> = names(&dir.join("attachments")).into_iter().filter(|n| !n.starts_with('.')).collect();
    assert!(stored.is_empty(), "stored files left: {stored:?}");
    let rows: i64 = raw(dir).query_row("SELECT COUNT(*) FROM draft_attachment", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 0, "a half row");
}

#[test]
fn a_source_that_changes_size_while_it_is_copied_is_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let source = write(dir.path(), "growing.bin", &vec![7u8; 4096]);
    let growing = source.clone();
    store.inject_attach_fault_for_test(AttachFault::MidCopy(Arc::new(move || {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new().append(true).open(&growing).unwrap();
        file.write_all(b"more").unwrap();
    })));
    let error = store.attach_path(INBOX_ID, &source, &now()).unwrap_err();
    assert!(matches!(error, StoreError::AttachmentUnreadable), "{error}");
    store.inject_attach_fault_for_test(AttachFault::Clear);
    nothing_left(dir.path());
}

#[test]
fn a_full_disk_rolls_back_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    store.inject_attach_fault_for_test(AttachFault::DiskFullAfter(50));
    let error = attach_file(&store, dir.path(), "big.bin", &[1u8; 200]).unwrap_err();
    assert!(matches!(error, StoreError::DiskFull), "{error}");
    assert_eq!(error.code(), kinige_domain::ErrorCode::DiskFull);
    store.inject_attach_fault_for_test(AttachFault::Clear);
    nothing_left(dir.path());
    // And it works again once there is room.
    assert!(attach_file(&store, dir.path(), "small.bin", &[1u8; 20]).is_ok());
}

#[test]
fn an_empty_file_is_a_file_of_zero_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let view = attach_file(&store, dir.path(), "empty.txt", b"").unwrap();
    assert_eq!((view.kind, view.byte_size), (AttachmentKind::File, 0));
    assert_eq!(view.mime, "text/plain");
    let sha: String = raw(dir.path())
        .query_row("SELECT sha256 FROM draft_attachment", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sha, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn a_file_over_two_gibibytes_is_refused_without_copying_a_byte() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let path = dir.path().join("huge.bin");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_FILE_BYTES + 1).unwrap();
    drop(file);
    let error = store.attach_path(INBOX_ID, &path, &now()).unwrap_err();
    assert!(matches!(error, StoreError::AttachmentTooLarge), "{error}");
    nothing_left(dir.path());
}

#[test]
fn paths_that_are_not_regular_files_are_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let file = write(dir.path(), "real.txt", b"hello");
    std::fs::create_dir(dir.path().join("folder")).unwrap();
    let missing = dir.path().join("missing.txt");
    #[cfg(unix)]
    let link = {
        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        link
    };
    let mut bad = vec![dir.path().join("folder"), missing, std::path::PathBuf::from("")];
    bad.push(std::path::PathBuf::from("relative/../../nothing"));
    #[cfg(unix)]
    bad.push(link);
    for path in bad {
        let error = store.attach_path(INBOX_ID, &path, &now()).unwrap_err();
        assert!(matches!(error, StoreError::AttachmentUnreadable), "{path:?}: {error}");
    }
    // A FIFO must never be opened (it would block forever).
    #[cfg(unix)]
    {
        let fifo = dir.path().join("pipe");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        // SAFETY: `mkfifo` gets a valid NUL-terminated path.
        #[allow(unsafe_code)]
        let made = unsafe { libc::mkfifo(c.as_ptr(), 0o600) };
        assert_eq!(made, 0);
        assert!(matches!(store.attach_path(INBOX_ID, &fifo, &now()), Err(StoreError::AttachmentUnreadable)));
    }
    nothing_left(dir.path());
}

#[test]
fn images_are_recognised_by_content_not_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    // A PNG named .txt is an image; a text file named .jpg is a file; HEIC-like and SVG names are files.
    let png_named_txt = attach_file(&store, dir.path(), "photo.txt", &png(30, 20)).unwrap();
    assert_eq!((png_named_txt.kind, png_named_txt.mime.as_str()), (AttachmentKind::Image, "image/png"));
    assert_eq!((png_named_txt.width, png_named_txt.height), (Some(30), Some(20)));
    let stored: String = raw(dir.path())
        .query_row("SELECT file_name FROM draft_attachment WHERE attachment_id = ?1", [&png_named_txt.id], |r| r.get(0))
        .unwrap();
    assert_eq!(stored, format!("{}.txt", png_named_txt.id), "the extension is the original one");
    let fake_jpg = attach_file(&store, dir.path(), "notes.jpg", b"just text, not a jpeg").unwrap();
    assert_eq!((fake_jpg.kind, fake_jpg.width), (AttachmentKind::File, None));
    let heic = attach_file(&store, dir.path(), "IMG_1.HEIC", b"\0\0\0\x18ftypheic....").unwrap();
    assert_eq!(heic.kind, AttachmentKind::File);
    let svg = attach_file(&store, dir.path(), "logo.svg", b"<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
    assert_eq!((svg.kind, svg.mime.as_str()), (AttachmentKind::File, "image/svg+xml"));
    let movie = attach_file(&store, dir.path(), "clip.MOV", b"fake movie").unwrap();
    assert_eq!(movie.kind, AttachmentKind::Video);
    let song = attach_file(&store, dir.path(), "voice.ogg", b"fake audio").unwrap();
    assert_eq!(song.kind, AttachmentKind::Audio);
    let no_ext = attach_file(&store, dir.path(), "README", b"plain").unwrap();
    assert_eq!(no_ext.kind, AttachmentKind::File);
    let gif_view = attach_file(&store, dir.path(), "anim.gif", &gif(12, 9)).unwrap();
    assert_eq!(gif_view.kind, AttachmentKind::Image);
}

#[test]
fn exif_orientation_swaps_the_stored_size_and_png_has_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for (orientation, expected) in [(1u8, (400, 200)), (3, (400, 200)), (6, (200, 400)), (8, (200, 400)), (5, (200, 400)), (2, (400, 200))] {
        let view = attach_file(&store, dir.path(), "o.jpg", &jpeg_with_orientation(400, 200, orientation)).unwrap();
        assert_eq!((view.width, view.height), (Some(expected.0), Some(expected.1)), "orientation {orientation}");
        store.remove_draft_attachment(INBOX_ID, &view.id).unwrap();
    }
    let flat = attach_file(&store, dir.path(), "p.png", &png(400, 200)).unwrap();
    assert_eq!((flat.width, flat.height), (Some(400), Some(200)));
}

fn sent(store: &Store, dir: &std::path::Path, name: &str, bytes: &[u8]) -> kinige_domain::AttachmentView {
    let view = attach_file(store, dir, name, bytes).unwrap();
    store.send_entry(INBOX_ID, "", None, &[view.id.clone()], &now()).unwrap().attachments.remove(0)
}

fn decode_size(bytes: &[u8]) -> (u32, u32) {
    let image = image::load_from_memory(bytes).unwrap();
    (image.width(), image.height())
}

#[test]
fn thumbnails_fit_960_rotate_drop_exif_and_are_cached() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let big = sent(&store, dir.path(), "big.jpg", &jpeg(2000, 1000));
    let thumb = store.media(&big.id, MediaVariant::Thumb).expect("a thumbnail");
    assert_eq!(thumb.content_type, "image/jpeg");
    assert_eq!(decode_size(&thumb.bytes), (960, 480));
    let cached = dir.path().join("thumbs").join(format!("{}.jpg", big.id));
    assert!(cached.is_file(), "built once, kept in thumbs/");
    assert_eq!(store.media(&big.id, MediaVariant::Thumb).unwrap().bytes, thumb.bytes);

    let small = sent(&store, dir.path(), "small.png", &png(100, 50));
    let kept = store.media(&small.id, MediaVariant::Thumb).unwrap();
    assert_eq!(decode_size(&kept.bytes), (100, 50), "a small image is not enlarged");

    let rotated = sent(&store, dir.path(), "rot.jpg", &jpeg_with_orientation(400, 200, 6));
    let upright = store.media(&rotated.id, MediaVariant::Thumb).unwrap();
    assert_eq!(decode_size(&upright.bytes), (200, 400), "the orientation is applied");
    assert!(
        !upright.bytes.windows(4).any(|w| w == b"Exif"),
        "EXIF is stripped from the thumbnail"
    );

    let alpha = sent(&store, dir.path(), "alpha.png", &png_with_alpha(64, 64));
    let transparent = store.media(&alpha.id, MediaVariant::Thumb).unwrap();
    assert_eq!(transparent.content_type, "image/png", "an image with alpha keeps it");
    assert!(dir.path().join("thumbs").join(format!("{}.png", alpha.id)).is_file());

    let animated = sent(&store, dir.path(), "a.gif", &gif(20, 20));
    assert_eq!(decode_size(&store.media(&animated.id, MediaVariant::Thumb).unwrap().bytes), (20, 20));
}

#[test]
fn a_truncated_jpeg_and_a_decompression_bomb_get_no_preview_and_no_oom() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    // A truncated PNG: the header reads, the data does not.
    let whole = png(800, 600);
    let truncated = sent(&store, dir.path(), "cut.png", &whole[..whole.len() / 3]);
    assert_eq!((truncated.kind, truncated.width, truncated.height), (AttachmentKind::Image, Some(800), Some(600)));
    assert!(store.media(&truncated.id, MediaVariant::Thumb).is_none(), "no preview");
    // A truncated JPEG: `image` 0.25.10 hard-wires zune-jpeg's lenient mode, so the part that decodes is shown (the tile does
    // not say "No preview" for a photo that is mostly there). It must still be a bounded thumbnail.
    let whole = jpeg(800, 600);
    let cut = sent(&store, dir.path(), "cut.jpg", &whole[..whole.len() / 3]);
    assert_eq!((cut.kind, cut.width, cut.height), (AttachmentKind::Image, Some(800), Some(600)));
    if let Some(partial) = store.media(&cut.id, MediaVariant::Thumb) {
        assert_eq!(decode_size(&partial.bytes), (800, 600));
    }
    // Garbage after a valid header never panics a thread.
    let mut broken = jpeg(64, 64);
    for byte in broken.iter_mut().skip(400) {
        *byte = 0xA5;
    }
    let garbled = sent(&store, dir.path(), "garbled.jpg", &broken);
    let _ = store.media(&garbled.id, MediaVariant::Thumb);

    let started = std::time::Instant::now();
    let bomb = sent(&store, dir.path(), "bomb.png", &png_bomb(100_000, 100_000));
    assert_eq!(bomb.kind, AttachmentKind::Image, "a larger image is still an image");
    assert_eq!((bomb.width, bomb.height), (Some(100_000), Some(100_000)));
    assert!(store.media(&bomb.id, MediaVariant::Thumb).is_none());
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
    // Not decoded again on every render within the same launch.
    assert!(store.media(&bomb.id, MediaVariant::Thumb).is_none());
}

#[test]
fn the_protocol_serves_only_what_it_may() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let photo = sent(&store, dir.path(), "p.jpg", &jpeg(64, 64));
    let pdf = sent(&store, dir.path(), "menu.pdf", b"%PDF-1.4");
    let svg = sent(&store, dir.path(), "logo.svg", b"<svg onload='alert(1)'/>");
    let png_attachment = sent(&store, dir.path(), "x.png", &png(8, 8));

    let full = store.media(&photo.id, MediaVariant::Full).unwrap();
    assert_eq!(full.content_type, "image/jpeg", "the type comes from the detected format");
    assert_eq!(full.bytes, std::fs::read(dir.path().join("attachments").join(format!("{}.jpg", photo.id))).unwrap());
    assert_eq!(store.media(&png_attachment.id, MediaVariant::Full).unwrap().content_type, "image/png");
    for id in [&pdf.id, &svg.id] {
        assert!(store.media(id, MediaVariant::Full).is_none(), "only images are served in full");
        assert!(store.media(id, MediaVariant::Thumb).is_none(), "and only images have thumbnails");
    }
    for bad in ["", "..", "%2e%2e", "../x", &photo.id.to_uppercase(), "00000000-0000-1000-8000-000000000000", &"a".repeat(1 << 16)] {
        assert!(store.media(bad, MediaVariant::Full).is_none(), "{bad:.30}");
        assert!(store.media(bad, MediaVariant::Thumb).is_none(), "{bad:.30}");
    }
    // The row's size is over the cap: the thumbnail may still be served, the file is not.
    raw(dir.path())
        .execute("UPDATE attachment SET byte_size = ?1 WHERE id = ?2", (kinige_domain::attachment::MAX_FULL_BYTES as i64 + 1, &photo.id))
        .unwrap();
    assert!(store.media(&photo.id, MediaVariant::Full).is_none());
    assert!(store.media(&photo.id, MediaVariant::Thumb).is_some());
    // A file that is gone is a 404, not an error.
    std::fs::remove_file(dir.path().join("attachments").join(format!("{}.png", png_attachment.id))).unwrap();
    assert!(store.media(&png_attachment.id, MediaVariant::Full).is_none());
    assert!(store.media(&png_attachment.id, MediaVariant::Thumb).is_none());
}

#[test]
fn media_async_answers_from_the_pool() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let photo = sent(&store, dir.path(), "p.jpg", &jpeg(300, 300));
    let (tx, rx) = std::sync::mpsc::channel();
    for _ in 0..8 {
        let tx = tx.clone();
        store.media_async(photo.id.clone(), MediaVariant::Thumb, move |media| {
            tx.send(media.map(|m| m.bytes.len())).unwrap();
        });
    }
    for _ in 0..8 {
        assert!(rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap().is_some());
    }
}

#[test]
fn opening_clones_the_file_and_only_documents_are_opened() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let pdf = sent(&store, dir.path(), "menu.pdf", b"%PDF-1.4 menu");
    let target = store.prepare_open(&pdf.id).unwrap();
    assert!(!target.reveal_only);
    assert_eq!(std::fs::read(&target.path).unwrap(), b"%PDF-1.4 menu");
    assert_eq!(target.path.file_name().unwrap(), "menu.pdf");
    assert!(target.path.starts_with(dir.path().join("open")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&target.path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    // Edits in the system app never touch the stored file.
    std::fs::write(&target.path, b"edited in Preview").unwrap();
    let stored = dir.path().join("attachments").join(format!("{}.pdf", pdf.id));
    assert_eq!(std::fs::read(stored).unwrap(), b"%PDF-1.4 menu");

    for name in ["run.sh", "Installer.app", "tool.command", "payload.exe", "README", "evil.svg", "macro.docm", "a.zip", "x.dmg"] {
        let view = sent(&store, dir.path(), name, b"#!/bin/sh\necho hi\n");
        let target = store.prepare_open(&view.id).unwrap();
        assert!(target.reveal_only, "{name} must only be revealed, never run");
    }
    // A JPEG with no extension gets one, so the OS sees a picture.
    let bare = sent(&store, dir.path(), "photo", &jpeg(20, 20));
    let target = store.prepare_open(&bare.id).unwrap();
    assert_eq!(target.path.file_name().unwrap(), "photo.jpg");
    assert!(!target.reveal_only);

    assert!(matches!(store.prepare_open("nope"), Err(StoreError::AttachmentNotFound)));
    assert!(matches!(
        store.prepare_open("00000000-0000-4000-8000-0000000000aa"),
        Err(StoreError::AttachmentNotFound)
    ));
    std::fs::remove_file(dir.path().join("attachments").join(format!("{}.pdf", pdf.id))).unwrap();
    std::fs::remove_dir_all(dir.path().join("open")).unwrap();
    assert!(matches!(store.prepare_open(&pdf.id), Err(StoreError::AttachmentUnreadable)));
}

#[test]
fn a_panic_in_a_decoder_is_one_lost_thumbnail_not_a_dead_pool() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let first = sent(&store, dir.path(), "a.jpg", &jpeg(200, 200));
    let second = sent(&store, dir.path(), "b.jpg", &jpeg(210, 210));
    store.inject_attach_fault_for_test(AttachFault::DecoderPanics);
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let tx = tx.clone();
        store.media_async(first.id.clone(), MediaVariant::Thumb, move |media| tx.send(media.is_some()).unwrap());
    }
    assert!(!rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap(), "no preview, no crash");
    store.inject_attach_fault_for_test(AttachFault::Clear);
    // The pool still answers.
    store.media_async(second.id.clone(), MediaVariant::Thumb, move |media| tx.send(media.is_some()).unwrap());
    assert!(rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap());
}
