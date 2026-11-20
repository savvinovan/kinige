//! Slice 5c demo data (KINIG-151): stickers and GIFs drawn here with the `image` crate (no sticker, GIF or picture of anyone else's, rules 9 and 10).
//! Only with `KINIGE_SEED_STICKERS=1`, so earlier scenarios keep their data.
//!
//! It puts a pack "Cats" of eight drawn stickers into the library and sends some of them into Inbox (a bare one, one with reactions, one as a reply),
//! sends two GIFs (a small one that autoplays and a large one, over two megapixels, that shows a badge), and, when `KINIGE_SEED_FAKE_BOT_DIR` names a
//! folder, writes there what the scenario's fake Bot API server (`scripts/screens/fake-bot.py`) serves: the files `S0.webp` ... of the pack the scenario adds.

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, ImageEncoder, Rgba, RgbaImage};
use kinige_domain::Language;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::stickers::PackSource;
use kinige_domain::telegram::parse_export;
use kinige_store::{NewPack, Now, PackItem, Store};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const MINUTE: i64 = 60_000;
/// The face colors of the drawn stickers.
const FUR: [[u8; 3]; 8] = [
    [240, 170, 70],
    [120, 120, 130],
    [60, 60, 70],
    [235, 225, 210],
    [200, 120, 90],
    [150, 170, 120],
    [90, 130, 190],
    [210, 150, 190],
];
const EMOJI: [&str; 8] = ["😺", "😸", "😻", "🙀", "😽", "😼", "🐱", "😹"];

/// A drawn cat face on a transparent square: a disc, two ears, two eyes, a nose; `tint` varies the colors.
pub fn sticker_png_or_webp(side: u32, tint: usize, webp: bool) -> Vec<u8> {
    let fur = FUR[tint % FUR.len()];
    let image = RgbaImage::from_fn(side, side, |x, y| {
        let (cx, cy, r) = (side as f32 / 2.0, side as f32 * 0.56, side as f32 * 0.38);
        let (dx, dy) = (x as f32 - cx, y as f32 - cy);
        let face = dx * dx + dy * dy <= r * r;
        let ear = |ex: f32| {
            let (ax, ay) = (x as f32 - ex, y as f32 - side as f32 * 0.2);
            ax.abs() * 1.4 + ay.abs() <= side as f32 * 0.18 && ay < side as f32 * 0.12
        };
        let eye = |ex: f32| {
            let (ax, ay) = (x as f32 - ex, y as f32 - side as f32 * 0.5);
            ax * ax + ay * ay <= (side as f32 * 0.045).powi(2)
        };
        let nose = {
            let (ax, ay) = (x as f32 - cx, y as f32 - side as f32 * 0.62);
            ax.abs() * 1.2 + ay.abs() <= side as f32 * 0.04
        };
        if eye(cx - side as f32 * 0.14) || eye(cx + side as f32 * 0.14) {
            Rgba([30, 30, 40, 255])
        } else if nose {
            Rgba([230, 110, 130, 255])
        } else if face || ear(cx - side as f32 * 0.22) || ear(cx + side as f32 * 0.22) {
            Rgba([fur[0], fur[1], fur[2], 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    let mut bytes = Vec::new();
    if webp {
        image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
            .write_image(image.as_raw(), side, side, image::ExtendedColorType::Rgba8)
            .expect("webp");
    } else {
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(image.as_raw(), side, side, image::ExtendedColorType::Rgba8)
            .expect("png");
    }
    bytes
}

/// An animated GIF drawn here: a disc that moves over a gradient, `frames` frames of `width` x `height`.
fn gif(width: u32, height: u32, frames: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        encoder.set_repeat(Repeat::Infinite).expect("repeat");
        for frame in 0..frames {
            let shift = frame * width / frames.max(1);
            let image = RgbaImage::from_fn(width, height, |x, y| {
                let (dx, dy) = (
                    x as f32 - ((shift + width / 4) % width) as f32,
                    y as f32 - height as f32 / 2.0,
                );
                if dx * dx + dy * dy <= (height as f32 * 0.25).powi(2) {
                    Rgba([250, 200, 60, 255])
                } else {
                    Rgba([(x * 255 / width) as u8, 90, (y * 255 / height) as u8, 255])
                }
            });
            encoder
                .encode_frame(Frame::from_parts(
                    image,
                    0,
                    0,
                    Delay::from_numer_denom_ms(120, 1),
                ))
                .expect("gif frame");
        }
    }
    bytes
}

/// A synthetic Telegram export of "Saved Messages" with sticker messages, imported twice like a person re-importing after the update: first without
/// the files (the "not imported" lines of before 5c), one entry is then edited, then with them (the unedited ones become stickers, the edited one
/// keeps its text and gets the sticker as a picture). A TGS and a file the export did not download stay placeholders.
fn import_export_stickers(store: &Store, now: &Now, files: &Path) {
    let root = files.join("tg-export");
    std::fs::create_dir_all(root.join("stickers")).expect("export folder");
    for (name, tint) in [("a.webp", 3), ("b.webp", 5), ("c.webp", 6)] {
        std::fs::write(
            root.join("stickers").join(name),
            sticker_png_or_webp(512, tint, true),
        )
        .expect("sticker file");
    }
    std::fs::write(root.join("stickers/d.tgs"), b"\x1f\x8b synthetic").expect("tgs");
    let base = now.ms / 1000 - 3 * 86_400;
    let sticker = |id: i64, path: &str, mime: &str, emoji: &str| {
        format!(
            r#"{{"id":{id},"type":"message","date":"x","date_unixtime":"{unix}","from":"Я","from_id":"user1000001","file":"{path}","file_name":"sticker.webp","media_type":"sticker","sticker_emoji":"{emoji}","mime_type":"{mime}","file_size":1000,"width":512,"height":512,"text":"","text_entities":[]}}"#,
            unix = base + id * 60
        )
    };
    let messages = [
        sticker(11, "stickers/a.webp", "image/webp", "😺"),
        sticker(12, "stickers/b.webp", "image/webp", "😸"),
        sticker(13, "stickers/a.webp", "image/webp", "😺"),
        sticker(14, "stickers/c.webp", "image/webp", "🐱"),
        sticker(15, "stickers/d.tgs", "application/x-tgsticker", "🐟"),
        sticker(
            16,
            "(File not included. Change data exporting settings to download.)",
            "image/webp",
            "🐟",
        ),
    ];
    let json = format!(
        r#"{{"personal_information":{{"user_id":1000001}},"chats":{{"list":[{{"type":"saved_messages","id":1000001,"messages":[{}]}}]}}}}"#,
        messages.join(",")
    );
    let export = Arc::new(parse_export(&json).expect("the synthetic export parses"));
    let ids: Vec<String> = export.chats.iter().map(|c| c.id.clone()).collect();
    let run = |media: Option<&Path>| {
        store
            .import_telegram_with_media(
                export.clone(),
                ids.clone(),
                Language::Ru,
                now,
                |_| {},
                Arc::new(AtomicBool::new(false)),
                media,
            )
            .expect("import of the synthetic stickers");
    };
    run(None);
    let page = store
        .list_messages(INBOX_ID, None, 200, now)
        .expect("the Inbox feed");
    let edited = page
        .messages
        .iter()
        .find(|m| m.body_md.contains("🐱") && m.body_md.contains("не импортирован"))
        .expect("the imported entry of the fourth sticker");
    store
        .edit_message(
            &edited.id,
            &format!("{}\nЭтот кот мой любимый", edited.body_md),
            now,
        )
        .expect("edit");
    run(Some(root.as_path()));
}

pub fn seed_stickers(store: &Store, now: &Now, data: &Path) {
    let at = |minutes_ago: i64| Now::at(now.ms - minutes_ago * MINUTE, &now.tz);
    let files = data.join("seed-files");
    std::fs::create_dir_all(&files).expect("seed files folder");

    // The library: a pack of eight stickers.
    let stage = store.pack_stage().expect("stage");
    let items: Vec<PackItem> = (0..8)
        .map(|index| {
            std::fs::write(stage.file(index), sticker_png_or_webp(512, index, true)).expect("file");
            PackItem {
                index,
                emoji: Some(EMOJI[index].to_string()),
            }
        })
        .collect();
    store
        .add_sticker_pack(
            stage,
            NewPack {
                source: PackSource::TelegramLink,
                link_name: Some("Cats".into()),
                title: "Cats".into(),
                items,
            },
        )
        .expect("the pack");
    let library = store.sticker_library(false).expect("library");
    let ids: Vec<String> = library.packs[0]
        .stickers
        .iter()
        .map(|s| s.id.clone())
        .collect();

    // In Inbox: a bare one, one with reactions, a text entry and a sticker that replies to it.
    store
        .send_sticker(INBOX_ID, &ids[0], None, &at(60))
        .expect("sticker");
    let with_chips = store
        .send_sticker(INBOX_ID, &ids[1], None, &at(55))
        .expect("sticker");
    store
        .set_message_reaction(&with_chips.id, "\u{2764}\u{FE0F}", true, &at(54))
        .expect("reaction");
    store
        .set_message_reaction(&with_chips.id, "🔥", true, &at(53))
        .expect("reaction");
    let question = store
        .send_message(INBOX_ID, "Как тебе новые наклейки?", &at(50))
        .expect("entry");
    store
        .send_sticker(INBOX_ID, &ids[2], Some(&question.id), &at(48))
        .expect("reply sticker");

    // Two GIFs: a small one that plays, and a large one (over two megapixels) that shows its first frame with a badge.
    for (name, width, height, frames, ago) in [
        ("cat-small.gif", 320, 240, 8, 40),
        ("cat-large.gif", 1700, 1300, 3, 30),
    ] {
        let path: PathBuf = files.join(name);
        std::fs::write(&path, gif(width, height, frames)).expect("gif");
        let tile = store
            .attach_path(INBOX_ID, &path, &at(ago))
            .expect("attach the gif");
        store
            .send_entry(INBOX_ID, "", None, &[tile.id], &at(ago))
            .expect("send the gif");
    }

    import_export_stickers(store, now, &files);

    // What the scenario's fake server serves for the pack the scenario adds by its link.
    if let Some(dir) = std::env::var_os("KINIGE_SEED_FAKE_BOT_DIR") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("fake bot folder");
        for index in 0..6 {
            std::fs::write(
                dir.join(format!("S{index}.webp")),
                sticker_png_or_webp(512, index + 3, true),
            )
            .expect("fake file");
        }
    }
}
