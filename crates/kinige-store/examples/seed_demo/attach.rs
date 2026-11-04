//! Slice 7 demo data: synthetic pictures drawn here (no third-party images), attached and sent through the store API like the
//! app does, plus a synthetic Telegram export with photos and a PDF that is imported twice (first without files, then with them,
//! so the "not imported" placeholders are replaced). Only with `KINIGE_SEED_ATTACH=1`, so earlier scenarios keep their data.

use chrono::Datelike;
use image::{ImageEncoder, Rgb, RgbImage};
use kinige_domain::Language;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::telegram::parse_export;
use kinige_store::{Now, Store};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const MINUTE: i64 = 60_000;
/// A fake but harmless document body: the card shows the name, the type and the size, never the content.
const PDF_BYTES: &[u8] = b"%PDF-1.4\n% synthetic file for screenshots\n%%EOF\n";
/// Sky, sun and hills colors of the drawn scenes: (sky top, sky bottom, hills).
const SCENES: [([u8; 3], [u8; 3], [u8; 3]); 5] = [
    ([40, 90, 170], [250, 190, 120], [30, 90, 60]),
    ([90, 60, 140], [240, 140, 120], [40, 60, 90]),
    ([20, 120, 150], [190, 235, 230], [60, 110, 70]),
    ([30, 40, 90], [120, 140, 200], [20, 40, 60]),
    ([210, 120, 70], [250, 220, 150], [110, 80, 50]),
];

pub fn seed_attachments(store: &Store, now: &Now, data: &Path, ideas_id: &str) {
    let files = data.join("seed-files");
    std::fs::create_dir_all(&files).expect("seed files folder");
    let at = |ms: i64| Now::at(now.ms + ms, &now.tz);
    let picture = |name: &str, width: u32, height: u32, scene: usize| -> PathBuf {
        let path = files.join(name);
        std::fs::write(&path, jpeg(width, height, scene)).expect("picture");
        path
    };
    let document = |name: &str| -> PathBuf {
        let path = files.join(name);
        std::fs::write(&path, PDF_BYTES).expect("document");
        path
    };
    let send = |chat: &str, text: &str, paths: &[PathBuf], ago: i64| {
        let ids: Vec<String> = paths
            .iter()
            .map(|path| store.attach_path(chat, path, &at(-ago)).expect("attach").id)
            .collect();
        store
            .send_entry(chat, text, None, &ids, &at(-ago))
            .expect("send entry with files")
    };

    // An album of four, a portrait photo with no text, a document with a caption, a mixed entry, one that will lose its file,
    // and one that goes to the trash.
    send(
        INBOX_ID,
        "Прогулка вдоль реки: четыре кадра с телефона.",
        &[
            picture("IMG_0001.jpg", 1600, 1000, 0),
            picture("IMG_0002.jpg", 1600, 1000, 1),
            picture("IMG_0003.jpg", 1000, 1600, 2),
            picture("IMG_0004.jpg", 1600, 1000, 3),
        ],
        40 * MINUTE,
    );
    send(
        INBOX_ID,
        "",
        &[picture("IMG_0005.jpg", 900, 1400, 4)],
        30 * MINUTE,
    );
    send(
        INBOX_ID,
        "Меню на субботу.",
        &[document("меню.pdf")],
        25 * MINUTE,
    );
    send(
        INBOX_ID,
        "Фото и два документа к поездке.",
        &[
            picture("IMG_0006.jpg", 1600, 1000, 2),
            document("билеты.pdf"),
            document("маршрут.pdf"),
        ],
        20 * MINUTE,
    );
    let lost = send(
        INBOX_ID,
        "Это фото потом пропадёт с диска.",
        &[picture("IMG_0007.jpg", 1600, 1000, 1)],
        15 * MINUTE,
    );
    let trashed = send(
        INBOX_ID,
        "Фото в корзине.",
        &[picture("IMG_0008.jpg", 1600, 1000, 3)],
        10 * MINUTE,
    );
    store
        .delete_message(&trashed.id, &at(-5 * MINUTE))
        .expect("to the trash");
    // Two more entries in the trash, each in a chat of its own so that a scenario can tell their buttons apart: the first is restored
    // and the second deleted for good.
    for (title, icon, files, text) in [
        (
            "Снимки",
            "📷",
            vec![picture("IMG_0010.jpg", 1600, 1000, 2)],
            "Фото на восстановление.",
        ),
        (
            "Альбом",
            "🖼️",
            vec![
                picture("IMG_0011.jpg", 1000, 1600, 3),
                document("смета.pdf"),
            ],
            "Фото и документ на удаление.",
        ),
    ] {
        let chat = store
            .create_chat(title, Some(icon), &at(-12 * MINUTE))
            .expect("chat");
        let entry = send(&chat.id, text, &files, 9 * MINUTE);
        store
            .delete_message(&entry.id, &at(-4 * MINUTE))
            .expect("to the trash");
    }
    // The pending tiles of the chat "Идеи": nothing is sent, the draft keeps them.
    for path in [
        picture("IMG_0009.jpg", 1600, 1000, 4),
        document("заметки.pdf"),
    ] {
        store
            .attach_path(ideas_id, &path, &at(-2 * MINUTE))
            .expect("tile");
    }
    // The file of one attachment disappears from the disk: the startup sweep marks the row `missing`.
    for attachment in &lost.attachments {
        let ext = attachment.filename.rsplit('.').next().unwrap_or("jpg");
        let stored = data
            .join("attachments")
            .join(format!("{}.{ext}", attachment.id));
        std::fs::remove_file(stored).expect("remove the stored file");
    }
    seed_telegram_photos(store, now, data);
}

/// A personal chat "Поездка" with three photos and a PDF, imported twice.
fn seed_telegram_photos(store: &Store, now: &Now, data: &Path) {
    let root = data.join("tg-media");
    for folder in ["photos", "files"] {
        std::fs::create_dir_all(root.join(folder)).expect("export folder");
    }
    for n in 1..=3usize {
        std::fs::write(
            root.join(format!("photos/photo_{n}.jpg")),
            jpeg(1280, 960, n),
        )
        .expect("export photo");
    }
    std::fs::write(root.join("files/Договор.pdf"), PDF_BYTES).expect("export file");
    let year = chrono::Utc::now().year().saturating_sub(3);
    let base = chrono::NaiveDate::from_ymd_opt(year, 6, 14)
        .and_then(|d| d.and_hms_opt(9, 0, 0))
        .map_or(1_600_000_000, |t| t.and_utc().timestamp());
    let message = |id: u64, extra: &str, text: &str| {
        let text = serde_json::to_string(text).expect("text");
        let entities = if text == "\"\"" {
            String::new()
        } else {
            format!(r#"{{"type":"plain","text":{text}}}"#)
        };
        format!(
            r#"{{"id":{id},"type":"message","date":"x","date_unixtime":"{at}","from":"Я","from_id":"user1000001",{extra}"text":{text},"text_entities":[{entities}]}}"#,
            at = base + 600 * id as i64
        )
    };
    let photo = |id: u64, n: usize, text: &str| {
        message(
            id,
            &format!(
                r#""photo":"photos/photo_{n}.jpg","photo_file_size":100,"width":1280,"height":960,"#
            ),
            text,
        )
    };
    let messages = [
        message(1, "", "Выехали рано утром"),
        photo(2, 1, "Первая остановка"),
        photo(3, 2, ""),
        photo(4, 3, "Вечер у воды"),
        message(
            5,
            r#""file":"files/Договор.pdf","file_name":"Договор.pdf","mime_type":"application/pdf","#,
            "",
        ),
    ];
    let json = format!(
        r#"{{"personal_information":{{"user_id":1000001}},"chats":{{"list":[{{"name":"Поездка","type":"personal_chat","id":3000003,"messages":[{}]}}]}}}}"#,
        messages.join(",")
    );
    let export = Arc::new(parse_export(&json).expect("the synthetic export parses"));
    let ids: Vec<String> = export.chats.iter().map(|c| c.id.clone()).collect();
    for media in [None, Some(root.as_path())] {
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
            .expect("import of the synthetic photos");
    }
}

/// A drawn landscape: a sky gradient, a sun and two hills. `scene` picks the palette.
fn jpeg(width: u32, height: u32, scene: usize) -> Vec<u8> {
    let (top, bottom, hills) = SCENES[scene % SCENES.len()];
    let mix = |a: u8, b: u8, t: f32| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
    let (w, h) = (width as f32, height as f32);
    let image = RgbImage::from_fn(width, height, |x, y| {
        let (fx, fy) = (x as f32 / w, y as f32 / h);
        let sun = ((fx - 0.7).powi(2) + ((fy - 0.3) * h / w).powi(2)).sqrt() < 0.09;
        let hill = fy > 0.72 - 0.1 * (fx * 6.0).sin() || fy > 0.85 - 0.12 * (fx * 3.0 + 1.0).cos();
        if sun {
            Rgb([255, 240, 200])
        } else if hill {
            Rgb([hills[0], hills[1], hills[2]])
        } else {
            Rgb([
                mix(top[0], bottom[0], fy),
                mix(top[1], bottom[1], fy),
                mix(top[2], bottom[2], fy),
            ])
        }
    });
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 82)
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .expect("encode");
    bytes
}
