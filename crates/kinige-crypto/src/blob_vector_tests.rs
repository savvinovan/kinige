//! The blobs of the test vectors (appendix §B items 1 and 4): sizes 0, 1 MiB - 1, 1 MiB and 1 MiB + 1 made by libsodium.js, which
//! Rust must open, and the damage that must be refused. The blobs are binary files in `vectors/blobs/`.

use crate::blob::{self, CHUNK};
use crate::secret::BlobKey;
use crate::vectors_tests::{bin, vectors};
use libsodium_rs::crypto_generichash::generichash;

#[test]
fn blobs_made_by_libsodium_js_open_and_ours_have_the_same_lengths() {
    let key = BlobKey::from_slice(&bin(&vectors()["blobs"][0], "blob_key")).expect("key");
    for entry in vectors()["blobs"].as_array().expect("blobs") {
        let size = entry["file_size"].as_u64().expect("size") as usize;
        let file: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let theirs = blob_bytes(size);
        assert_eq!(
            theirs.len() as u64,
            entry["blob_length"].as_u64().expect("length")
        );
        assert_eq!(
            blob::open_blob(&theirs, &key).expect("open"),
            file,
            "size {size}"
        );
        assert_eq!(
            generichash(&file, None, 32).expect("hash"),
            bin(entry, "file_blake2b256")
        );
        // The stream header is random, so the bytes differ, the length does not.
        assert_eq!(
            blob::seal_blob(&file, &key).expect("seal").len(),
            theirs.len(),
            "size {size}"
        );
    }
    assert!(blob_bytes(CHUNK - 1).len() < blob_bytes(CHUNK).len());
}

#[test]
fn a_damaged_vector_blob_is_refused_everywhere_it_can_be_damaged() {
    let key = BlobKey::from_slice(&bin(&vectors()["blobs"][0], "blob_key")).expect("key");
    let blob = blob_bytes(CHUNK);
    let first_frame = 24 + CHUNK + 17;
    assert!(blob.len() > first_frame, "two chunks");
    assert!(
        blob::open_blob(&blob[..first_frame], &key).is_err(),
        "without the final chunk"
    );
    assert!(
        blob::open_blob(&blob[..blob.len() - 1], &key).is_err(),
        "one byte short"
    );
    let mut longer = blob.clone();
    longer.push(0);
    assert!(
        blob::open_blob(&longer, &key).is_err(),
        "one byte after TAG_FINAL"
    );
    for position in [
        0,
        23,
        24,
        1000,
        first_frame - 1,
        first_frame,
        blob.len() - 1,
    ] {
        let mut changed = blob.clone();
        changed[position] ^= 0x80;
        assert!(blob::open_blob(&changed, &key).is_err(), "byte {position}");
    }
}

fn blob_bytes(size: usize) -> Vec<u8> {
    match size {
        0 => include_bytes!("../vectors/blobs/blob-0.bin").to_vec(),
        1_048_575 => include_bytes!("../vectors/blobs/blob-1048575.bin").to_vec(),
        1_048_576 => include_bytes!("../vectors/blobs/blob-1048576.bin").to_vec(),
        _ => include_bytes!("../vectors/blobs/blob-1048577.bin").to_vec(),
    }
}
