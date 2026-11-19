//! Slice 7b demo data (KINIG-136): a small chat "Вложения" with hidden photos, a described photo, a renamed file card, an album with one hidden cell, a trashed
//! hidden photo, and three pending tiles (two hidden, one with its details, and a file) in the chat's draft. Synthetic pictures only. Only with `KINIGE_SEED_DETAILS=1`,
//! so earlier scenarios keep their data.

use image::{ImageEncoder, Rgb, RgbImage};
use kinige_domain::attachment::AttachmentDetailsChanges;
use kinige_store::{Now, Store};
use std::path::{Path, PathBuf};

const MINUTE: i64 = 60_000;
const PDF_BYTES: &[u8] = b"%PDF-1.4\n% synthetic file for screenshots\n%%EOF\n";

pub fn seed_details(store: &Store, now: &Now, data: &Path) {
    let files = data.join("seed-files-details");
    std::fs::create_dir_all(&files).expect("seed files folder");
    let at = |ago: i64| Now::at(now.ms - ago, &now.tz);
    let picture = |name: &str, scene: u8| -> PathBuf {
        let path = files.join(name);
        std::fs::write(&path, jpeg(1200, 800, scene)).expect("picture");
        path
    };
    let document = |name: &str| -> PathBuf {
        let path = files.join(name);
        std::fs::write(&path, PDF_BYTES).expect("document");
        path
    };
    let chat = store
        .create_chat("Вложения", None, &at(120 * MINUTE))
        .expect("chat")
        .id;
    let send = |text: &str, paths: &[PathBuf], ago: i64| {
        let ids: Vec<String> = paths
            .iter()
            .map(|path| store.attach_path(&chat, path, &at(ago)).expect("attach").id)
            .collect();
        store
            .send_entry(&chat, text, None, &ids, &at(ago))
            .expect("send entry")
    };
    let details =
        |id: &str, name: Option<&str>, description: Option<&str>, spoiler: bool, ago: i64| {
            let changes = AttachmentDetailsChanges {
                name: name.map(|n| Some(n.to_string())),
                description: description.map(|d| Some(d.to_string())),
                spoiler: spoiler.then_some(true),
            };
            store
                .set_attachment_details(id, &changes, &at(ago))
                .expect("details");
        };

    // 1. A hidden photo alone: a bare bubble with its pill on the cover.
    let alone = send("", &[picture("IMG_0101.jpg", 0)], 100 * MINUTE);
    details(&alone.attachments[0].id, None, None, true, 100 * MINUTE);
    // 2. A hidden photo with a name and a description: the cover and "Описание скрыто" until it is shown.
    let described = send("", &[picture("IMG_0102.jpg", 1)], 90 * MINUTE);
    details(
        &described.attachments[0].id,
        Some("закат"),
        Some("Закат над заливом, июль"),
        true,
        90 * MINUTE,
    );
    // 3. An album of four with one hidden cell.
    let album = send(
        "Прогулка вдоль реки.",
        &[
            picture("IMG_0103.jpg", 2),
            picture("IMG_0104.jpg", 3),
            picture("IMG_0105.jpg", 4),
            picture("IMG_0106.jpg", 0),
        ],
        80 * MINUTE,
    );
    details(&album.attachments[1].id, None, None, true, 80 * MINUTE);
    // 4. A shown photo with a description: the caption line.
    let caption = send("", &[picture("IMG_0107.jpg", 1)], 70 * MINUTE);
    details(
        &caption.attachments[0].id,
        Some("кофейня"),
        Some("Кофейня на углу, где мы договорились встретиться"),
        false,
        70 * MINUTE,
    );
    // 5. A file card with a new name and a description.
    let card = send("", &[document("scan0042.pdf")], 60 * MINUTE);
    details(
        &card.attachments[0].id,
        Some("Договор аренды"),
        Some("Подписанный экземпляр, страницы 1-6. Второй экземпляр лежит у арендодателя."),
        false,
        60 * MINUTE,
    );
    // 6. A plain photo with no details: the target of the dialog and of "Hide as spoiler".
    send("Обычное фото.", &[picture("IMG_0108.jpg", 2)], 50 * MINUTE);
    // 7. A hidden photo in the trash.
    let trashed = send("", &[picture("IMG_0109.jpg", 3)], 40 * MINUTE);
    details(&trashed.attachments[0].id, None, None, true, 40 * MINUTE);
    store
        .delete_message(&trashed.id, &at(30 * MINUTE))
        .expect("trash");

    // Pending tiles in the draft: a hidden photo with its details, and a file with a new name.
    let tile = store
        .attach_path(&chat, &picture("IMG_0110.jpg", 4), &at(20 * MINUTE))
        .expect("tile");
    store
        .set_draft_attachment_details(
            &tile.id,
            &AttachmentDetailsChanges {
                name: Some(Some("тайное".to_string())),
                description: Some(Some("Только для меня".to_string())),
                spoiler: Some(true),
            },
        )
        .expect("tile details");
    // A second hidden tile with no details: the scenario shows it, so one covered and one shown tile stand side by side.
    let second = store
        .attach_path(
            &chat,
            &picture("IMG_0111.jpg", 2),
            &at(19 * MINUTE + 30_000),
        )
        .expect("second tile");
    store
        .set_draft_attachment_details(
            &second.id,
            &AttachmentDetailsChanges {
                spoiler: Some(true),
                ..AttachmentDetailsChanges::default()
            },
        )
        .expect("second tile details");
    let file = store
        .attach_path(&chat, &document("notes.pdf"), &at(19 * MINUTE))
        .expect("file tile");
    store
        .set_draft_attachment_details(
            &file.id,
            &AttachmentDetailsChanges {
                name: Some(Some("заметки".to_string())),
                ..AttachmentDetailsChanges::default()
            },
        )
        .expect("file tile details");
}

/// A drawn scene: sky and hills, so that the picture is not a flat colour. The size is the one the entry reserves.
fn jpeg(width: u32, height: u32, scene: u8) -> Vec<u8> {
    let tint = [
        ([40, 90, 170], [250, 190, 120]),
        ([90, 60, 140], [240, 140, 120]),
        ([20, 120, 150], [190, 235, 230]),
        ([30, 40, 90], [120, 140, 200]),
        ([210, 120, 70], [250, 220, 150]),
    ];
    let (top, bottom) = tint
        .get(usize::from(scene) % tint.len())
        .copied()
        .unwrap_or(tint[0]);
    let mut image = RgbImage::new(width, height);
    for (_, y, pixel) in image.enumerate_pixels_mut() {
        let t = f64::from(y) / f64::from(height);
        let mix = |a: u8, b: u8| (f64::from(a) * (1.0 - t) + f64::from(b) * t) as u8;
        *pixel = if t > 0.7 {
            Rgb([30, 90, 60])
        } else {
            Rgb([
                mix(top[0], bottom[0]),
                mix(top[1], bottom[1]),
                mix(top[2], bottom[2]),
            ])
        };
    }
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 80)
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .expect("jpeg");
    bytes
}
