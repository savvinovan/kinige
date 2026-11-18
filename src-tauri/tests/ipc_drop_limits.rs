//! The limits of a drop that carries an image (KINIG-133, slice 7): a dragged TIFF is converted off the UI thread and never decoded past
//! the ADR-0014 limits, no image is read or held when no chat is open, a staged image of one drag is never attached by another, and
//! unclaimed drops cannot pile up image bytes. The payload kinds are in `ipc_drop_payloads.rs`; the helpers in `drop_support`.

// A whole integration-test root is test code (see `ipc_slice7.rs`).
#![cfg(test)]

mod common;
mod drop_support;

use common::*;
use drop_support::*;
use kinige_lib::attachments::DropTokens;
use kinige_lib::drag_source::DragImage;
use serde_json::{Value, json};
use tauri::Manager;

#[test]
fn a_dragged_tiff_is_converted_when_claimed_and_becomes_a_picture() {
    let dropper = Dropper::new();
    let source = FakeDrag {
        image: Some(DragImage::Tiff(small_tiff())),
        ..FakeDrag::default()
    };
    dropper.send(&enter(vec![]), &source);
    dropper.send(&drop_of(vec![]), &source);
    let tile = &dropper.claim(&dropper.token(1)).unwrap()["attached"][0];
    assert_eq!(tile["kind"], "image", "{tile}");
    assert_eq!(
        (tile["width"].as_u64(), tile["height"].as_u64()),
        (Some(24), Some(16))
    );
    assert!(
        tile["filename"].as_str().unwrap().ends_with(".png"),
        "{tile}"
    );
}

#[test]
fn a_tiff_bomb_is_never_decoded_and_stays_a_file_card_quickly() {
    // 6 MiB on the wire, 432 MB as pixels: the reviewer's probe. Over the limits of the conversion, so the bytes are attached as they
    // are (the store keeps a TIFF as a file card) and nothing is decoded: the claim returns at once.
    let dropper = Dropper::new();
    let bomb = kinige_lib::drag_tiff::packbits_tiff_for_test(12_000);
    assert!(bomb.len() < 8 * 1024 * 1024);
    let source = FakeDrag {
        image: Some(DragImage::Tiff(bomb)),
        ..FakeDrag::default()
    };
    // Entering with it staged costs a copy of the bytes, no decoding: the event handler returns at once too.
    let started = std::time::Instant::now();
    dropper.send(&enter(vec![]), &source);
    dropper.send(&drop_of(vec![]), &source);
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
    let outcome = dropper.claim(&dropper.token(1)).unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(outcome["attached"][0]["kind"], "file");
    assert_eq!(outcome["error"], Value::Null);
}

#[test]
fn a_broken_tiff_is_attached_like_a_pasted_one_and_never_panics() {
    let dropper = Dropper::new();
    let source = FakeDrag {
        image: Some(DragImage::Tiff(b"II*\0 broken".to_vec())),
        ..FakeDrag::default()
    };
    dropper.send(&drop_of(vec![]), &source);
    let dropped = dropper.claim(&dropper.token(1)).unwrap();
    let pasted = raw_paste(&dropper.main, b"II*\0 broken".to_vec()).unwrap();
    assert_eq!(dropped["attached"][0]["kind"], pasted["kind"]);
}

#[test]
fn with_no_chat_open_nothing_is_read_from_the_drag() {
    let dropper = Dropper::without_chat();
    let source = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        ..FakeDrag::default()
    };
    dropper.send(&enter(vec![]), &source);
    dropper.send(&drop_of(vec![]), &source);
    assert_eq!(
        (source.image_reads.get(), source.path_reads.get()),
        (0, 0),
        "no chat is open: the pasteboard is not touched"
    );
    let tokens = dropper.fixture.app.state::<DropTokens>();
    assert_eq!(tokens.images_held_for_test(), (0, false));
    // The token is still left, and claiming it says there was nothing.
    assert_eq!(
        dropper.claim(&dropper.token(1)).unwrap()["error"],
        "drop_not_supported"
    );
}

#[test]
fn closing_the_chat_forgets_a_staged_image() {
    let dropper = Dropper::new();
    let source = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        ..FakeDrag::default()
    };
    dropper.send(&enter(vec![]), &source);
    let tokens = dropper.fixture.app.state::<DropTokens>();
    assert_eq!(tokens.images_held_for_test(), (0, true));
    dropper.set_target(false, 2);
    assert_eq!(tokens.images_held_for_test(), (0, false));
}

#[test]
fn an_image_left_over_from_a_cancelled_drag_is_never_attached_by_a_later_drop() {
    // Esc over the window may send no `Leave` (wry implements only `draggingExited:`): the staged image of drag 1 is still there when
    // drag 2 drops. Its generation differs, so the drop reads drag 2 itself.
    let dropper = Dropper::new();
    let first = FakeDrag {
        image: Some(DragImage::Bytes(b"\xff\xd8first".to_vec())),
        generation: 1,
        ..FakeDrag::default()
    };
    let second = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        generation: 2,
        ..FakeDrag::default()
    };
    dropper.send(&enter(vec![]), &first);
    dropper.send(&drop_of(vec![]), &second);
    assert_eq!(second.image_reads.get(), 1, "the stale image was not used");
    let tile = &dropper.claim(&dropper.token(1)).unwrap()["attached"][0];
    assert_eq!(tile["kind"], "image");
    assert!(
        tile["filename"].as_str().unwrap().ends_with(".png"),
        "{tile}"
    );
}

#[test]
fn a_staged_image_is_used_once() {
    let dropper = Dropper::new();
    let source = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        generation: 7,
        ..FakeDrag::default()
    };
    dropper.send(&enter(vec![]), &source);
    dropper.send(&drop_of(vec![]), &source);
    assert_eq!(source.image_reads.get(), 1);
    // A second drop of the same pasteboard without a new enter reads again: nothing is left staged.
    dropper.send(&drop_of(vec![]), &source);
    assert_eq!(source.image_reads.get(), 2);
}

#[test]
fn only_the_newest_unclaimed_drop_holds_an_image() {
    let dropper = Dropper::new();
    let source = FakeDrag {
        image: Some(DragImage::Bytes(png_bytes())),
        ..FakeDrag::default()
    };
    let tokens = dropper.fixture.app.state::<DropTokens>();
    for n in 1..=5 {
        dropper.send(&drop_of(vec![]), &source);
        dropper.token(n);
    }
    assert_eq!(tokens.pending_for_test(), 5);
    assert_eq!(
        tokens.images_held_for_test(),
        (1, false),
        "5 unclaimed drops, one image"
    );
}

#[test]
fn calls_that_arrive_out_of_order_cannot_leave_a_stale_answer() {
    // The web view sends false (leaving a chat, call 2) and then true (the next chat, call 3); if they arrive the other way round, the
    // older one is ignored and a chat stays open for drops.
    let dropper = Dropper::new();
    let tokens = dropper.fixture.app.state::<DropTokens>();
    dropper.set_target(true, 3);
    dropper.set_target(false, 2);
    assert!(tokens.accepting(), "the older false was ignored");
    // The same number again, and an older one, change nothing; a newer one does.
    dropper.set_target(false, 3);
    dropper.set_target(false, 1);
    assert!(tokens.accepting());
    dropper.set_target(false, 4);
    assert!(!tokens.accepting());
    dropper.set_target(true, 3);
    assert!(!tokens.accepting(), "and an older true cannot reopen it");
}

#[test]
fn set_drop_target_takes_a_flag_a_number_and_nothing_else() {
    let dropper = Dropper::new();
    for bad in [
        json!({}),
        json!({"active": true}),
        json!({"seq": 5}),
        json!({"active": "yes", "seq": 5}),
        json!({"active": 1, "seq": 5}),
        json!({"active": null, "seq": 5}),
        json!({"active": [true], "seq": 5}),
        json!({"active": true, "seq": -1}),
        json!({"active": true, "seq": "9"}),
        json!({"active": true, "seq": 1.5}),
        json!({"active": true, "seq": 1.8446744073709552e19}),
    ] {
        assert!(
            call(&dropper.main, "set_drop_target", bad.clone()).is_err(),
            "{bad}"
        );
    }
    let tokens = dropper.fixture.app.state::<DropTokens>();
    assert!(tokens.accepting(), "a refused call changes nothing");
}
