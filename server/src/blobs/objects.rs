//! The object store behind one small interface: put a finished file, read a range, ask a size, delete, list. Two backends of the
//! `object_store` crate (ADR-0020): [`LocalFileSystem`] with `fsync` on (it writes a temporary file, syncs it, renames it and syncs the
//! directory), and [`AmazonS3`] for any S3-compatible store. The server does not know which one it has.
//!
//! An object is `<account_id>/<blob_id>` and never changes after it is put. Errors say what happened to the caller and carry no path.

use super::settings::{BlobSettings, S3Settings};
use crate::hashing::Hasher;
use crate::rules::blob::ByteRange;
use futures_util::stream::{BoxStream, StreamExt};
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use object_store::prefix::PrefixStore;
use object_store::{GetOptions, GetRange, ObjectStore, ObjectStoreExt, PutPayload, WriteMultipart};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Files up to this size go in one request, larger ones as a multipart upload in parts of this size.
const PART: usize = 8 << 20;
/// How many parts of a multipart upload may be in flight.
const PARTS_IN_FLIGHT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectError {
    NotFound,
    /// The store or the local file failed; the category is logged where it happened.
    Failed,
}

/// One object as a listing shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub key: String,
    pub size: u64,
    /// Milliseconds since the Unix epoch.
    pub modified_ms: u64,
}

#[derive(Clone)]
pub struct Objects {
    store: Arc<dyn ObjectStore>,
}

impl Objects {
    pub fn new(store: Arc<dyn ObjectStore>) -> Objects {
        Objects { store }
    }

    /// The store the settings name. For a disk store the directory is created.
    pub fn open(settings: &BlobSettings) -> Result<Objects, String> {
        match &settings.s3 {
            Some(s3) => Objects::s3(s3),
            None => Objects::disk(&settings.objects_dir()),
        }
    }

    pub fn disk(dir: &Path) -> Result<Objects, String> {
        std::fs::create_dir_all(dir)
            .map_err(|_| "the blob directory cannot be created".to_string())?;
        let store = LocalFileSystem::new_with_prefix(dir)
            .map_err(|_| "the blob directory cannot be used".to_string())?
            .with_fsync(true);
        Ok(Objects::new(Arc::new(store)))
    }

    pub fn s3(settings: &S3Settings) -> Result<Objects, String> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(&settings.bucket)
            .with_region(&settings.region)
            .with_access_key_id(&settings.access_key)
            .with_secret_access_key(&settings.secret_key)
            .with_allow_http(settings.allow_http)
            .with_virtual_hosted_style_request(false);
        if let Some(endpoint) = &settings.endpoint {
            builder = builder.with_endpoint(endpoint);
        }
        let store = builder
            .build()
            .map_err(|_| "the S3 settings are not valid".to_string())?;
        Ok(Objects::new(Arc::new(store)))
    }

    /// The same store with every key under `prefix` (tests share one bucket and must not see each other's objects).
    pub fn with_prefix(self, prefix: &str) -> Objects {
        Objects::new(Arc::new(PrefixStore::new(self.store, prefix)))
    }

    /// Puts the file at `path` as the object `key`. Returns when the object is durable and visible.
    pub async fn put_file(&self, key: &str, path: &Path) -> Result<(), ObjectError> {
        let location = location(key)?;
        let mut file = File::open(path).await.map_err(failed("open"))?;
        let len = file.metadata().await.map_err(failed("stat"))?.len();
        if len <= u64::try_from(PART).unwrap_or(0) {
            let mut bytes = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
            file.read_to_end(&mut bytes).await.map_err(failed("read"))?;
            self.store
                .put(&location, PutPayload::from(bytes))
                .await
                .map_err(store_failed)?;
            return Ok(());
        }
        let upload = self
            .store
            .put_multipart(&location)
            .await
            .map_err(store_failed)?;
        let mut writer = WriteMultipart::new_with_chunk_size(upload, PART);
        let mut buffer = vec![0u8; 1 << 20];
        loop {
            let read = file.read(&mut buffer).await.map_err(failed("read"))?;
            let Some(filled) = buffer.get(..read).filter(|filled| !filled.is_empty()) else {
                break;
            };
            writer
                .wait_for_capacity(PARTS_IN_FLIGHT)
                .await
                .map_err(store_failed)?;
            writer.write(filled);
        }
        writer.finish().await.map_err(store_failed)?;
        Ok(())
    }

    /// The size of the object, `None` if there is none.
    pub async fn size(&self, key: &str) -> Result<Option<u64>, ObjectError> {
        match self.store.head(&location(key)?).await {
            Ok(meta) => Ok(Some(meta.size)),
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(error) => Err(store_failed(error)),
        }
    }

    /// The bytes of `range` (inclusive) as a stream. `NotFound` if the object is gone.
    pub async fn read(
        &self,
        key: &str,
        range: ByteRange,
    ) -> Result<BoxStream<'static, Result<axum::body::Bytes, ObjectError>>, ObjectError> {
        let options = GetOptions {
            range: Some(GetRange::Bounded(range.start..range.end.saturating_add(1))),
            ..GetOptions::default()
        };
        let result = self
            .store
            .get_opts(&location(key)?, options)
            .await
            .map_err(|error| match error {
                object_store::Error::NotFound { .. } => ObjectError::NotFound,
                other => store_failed(other),
            })?;
        Ok(result
            .into_stream()
            .map(|chunk| chunk.map_err(store_failed))
            .boxed())
    }

    /// Deletes the object. An object that is not there is a success.
    pub async fn delete(&self, key: &str) -> Result<(), ObjectError> {
        match self.store.delete(&location(key)?).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(store_failed(error)),
        }
    }

    /// Every object, in no particular order.
    pub async fn list(&self) -> Result<Vec<Listed>, ObjectError> {
        let mut stream = self.store.list(None);
        let mut listed = Vec::new();
        while let Some(meta) = stream.next().await {
            let meta = meta.map_err(store_failed)?;
            listed.push(Listed {
                key: meta.location.to_string(),
                size: meta.size,
                modified_ms: u64::try_from(meta.last_modified.timestamp_millis()).unwrap_or(0),
            });
        }
        Ok(listed)
    }

    /// Writes the object into the file `dest` (created, truncated, synced) and returns its size and hash.
    pub async fn download(&self, key: &str, dest: &Path) -> Result<(u64, String), ObjectError> {
        let size = self.size(key).await?.ok_or(ObjectError::NotFound)?;
        let mut file = File::create(dest).await.map_err(failed("create"))?;
        let mut hasher = Hasher::new();
        let mut written = 0u64;
        if size > 0 {
            let whole = ByteRange {
                start: 0,
                end: size.saturating_sub(1),
            };
            let mut stream = self.read(key, whole).await?;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                hasher.update(&chunk);
                file.write_all(&chunk).await.map_err(failed("write"))?;
                written = written.saturating_add(u64::try_from(chunk.len()).unwrap_or(0));
            }
        }
        file.sync_all().await.map_err(failed("sync"))?;
        if written != size {
            log::error!("an object came back with another length than its size");
            return Err(ObjectError::Failed);
        }
        Ok((size, hasher.finish()))
    }
}

fn location(key: &str) -> Result<ObjectPath, ObjectError> {
    ObjectPath::parse(key).map_err(|_| {
        log::error!("an object key does not parse");
        ObjectError::Failed
    })
}

fn failed(what: &'static str) -> impl Fn(std::io::Error) -> ObjectError {
    move |error| {
        log::error!("a blob file failed ({what}): {:?}", error.kind());
        ObjectError::Failed
    }
}

/// The error text of the store can carry the URL it called; only the category is logged.
fn store_failed(error: object_store::Error) -> ObjectError {
    let category = match error {
        object_store::Error::NotFound { .. } => "not found",
        object_store::Error::Precondition { .. } => "precondition",
        object_store::Error::PermissionDenied { .. }
        | object_store::Error::Unauthenticated { .. } => "permission",
        object_store::Error::Generic { .. } => "generic",
        _ => "other",
    };
    log::error!("the object store failed: {category}");
    ObjectError::Failed
}
