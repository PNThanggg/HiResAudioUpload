//! Track catalog and library CRUD over an object store (R2 layout in the design spec).
//! Write order: objects first, catalog last. Delete order: catalog first, objects after.

use crate::pipeline::probe::sniff_image;
use crate::pipeline::{ANALYSIS_VERSION, ProcessedTrack, sha256_hex};
use async_trait::async_trait;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const CATALOG_KEY: &str = "catalog.json";
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const NO_CACHE: &str = "no-cache";
const JSON: &str = "application/json";

#[async_trait]
pub trait Storage: Send + Sync {
    async fn put(
        &self,
        key: &str,
        body: Vec<u8>,
        content_type: &str,
        cache_control: &str,
    ) -> anyhow::Result<()>;
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>>;
    async fn delete(&self, key: &str) -> anyhow::Result<()>;
}

type StoredObject = (Vec<u8>, String, String);

/// In-memory `Storage` for tests and UI mocks.
#[derive(Default)]
pub struct MemoryStorage {
    objects: Mutex<BTreeMap<String, StoredObject>>,
    /// When set, puts whose key contains this substring fail.
    pub fail_puts_matching: Mutex<Option<String>>,
}

impl MemoryStorage {
    pub fn keys(&self) -> Vec<String> {
        self.objects.lock().unwrap().keys().cloned().collect()
    }

    /// `(content_type, cache_control)` of a stored object.
    pub fn object_headers(&self, key: &str) -> Option<(String, String)> {
        self.objects
            .lock()
            .unwrap()
            .get(key)
            .map(|(_, ct, cc)| (ct.clone(), cc.clone()))
    }
}

#[async_trait]
impl Storage for MemoryStorage {
    async fn put(
        &self,
        key: &str,
        body: Vec<u8>,
        content_type: &str,
        cache_control: &str,
    ) -> anyhow::Result<()> {
        if let Some(pat) = self.fail_puts_matching.lock().unwrap().as_deref()
            && key.contains(pat)
        {
            anyhow::bail!("injected put failure for {key}");
        }
        self.objects.lock().unwrap().insert(
            key.to_string(),
            (body, content_type.to_string(), cache_control.to_string()),
        );
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.objects.lock().unwrap().get(key).map(|(b, _, _)| b.clone()))
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.objects.lock().unwrap().remove(key);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSummary {
    pub id: String,
    pub source_sha256: String,
    pub audio_key: String,
    pub cover_key: Option<String>,
    pub audio_url: String,
    pub cover_url: Option<String>,
    pub size_bytes: u64,
    pub codec: String,
    pub bitrate: u32,
    pub vbr: bool,
    pub sample_rate: u32,
    pub channels: u8,
    pub duration_ms: u64,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub track: Option<String>,
    pub disc: Option<String>,
    pub integrated_lufs: f64,
    pub true_peak_dbtp: f64,
    pub lra: f64,
    pub effective_bandwidth_hz: Option<u32>,
    pub quality_hint: Option<String>,
    pub trim_start: Option<u32>,
    pub trim_end: Option<u32>,
    pub total_samples: Option<u64>,
    pub analysis_version: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackRecord {
    #[serde(flatten)]
    pub summary: TrackSummary,
    pub waveform: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub version: u32,
    pub updated_at: String,
    pub tracks: Vec<TrackSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagEdit {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub track: Option<String>,
    pub disc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "track", rename_all = "lowercase")]
pub enum AddOutcome {
    Added(TrackSummary),
    Duplicate(TrackSummary),
}

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("{0}")]
    CatalogCorrupt(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Storage(String),
}

impl LibraryError {
    pub fn code(&self) -> &'static str {
        match self {
            LibraryError::CatalogCorrupt(_) => "CATALOG_CORRUPT",
            LibraryError::NotFound(_) => "NOT_FOUND",
            LibraryError::Validation(_) => "VALIDATION",
            LibraryError::Storage(_) => "STORAGE",
        }
    }
}

impl Serialize for LibraryError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("LibraryError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<anyhow::Error> for LibraryError {
    fn from(e: anyhow::Error) -> Self {
        LibraryError::Storage(e.to_string())
    }
}

type Result<T> = std::result::Result<T, LibraryError>;

fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("RFC 3339 formatting of a UTC timestamp")
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn meta_key(id: &str) -> String {
    format!("meta/{id}.json")
}

fn cover_refs(tracks: &[TrackSummary], key: &str) -> bool {
    tracks.iter().any(|t| t.cover_key.as_deref() == Some(key))
}

pub struct Library {
    storage: Arc<dyn Storage>,
    public_base: String,
    lock: tokio::sync::Mutex<()>,
}

impl Library {
    pub fn new(storage: Arc<dyn Storage>, public_base: String) -> Self {
        Self {
            storage,
            public_base,
            lock: tokio::sync::Mutex::new(()),
        }
    }

    fn url(&self, key: &str) -> String {
        format!("{}/{}", self.public_base, key)
    }

    async fn load(&self) -> Result<Catalog> {
        match self.storage.get(CATALOG_KEY).await? {
            None => Ok(Catalog {
                version: 1,
                updated_at: now(),
                tracks: Vec::new(),
            }),
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| LibraryError::CatalogCorrupt(format!("catalog.json: {e}"))),
        }
    }

    async fn save(&self, mut cat: Catalog) -> Result<()> {
        cat.updated_at = now();
        let body = serde_json::to_vec(&cat).map_err(|e| LibraryError::Storage(e.to_string()))?;
        self.storage.put(CATALOG_KEY, body, JSON, NO_CACHE).await?;
        Ok(())
    }

    async fn put_meta(&self, rec: &TrackRecord) -> Result<()> {
        let body = serde_json::to_vec(rec).map_err(|e| LibraryError::Storage(e.to_string()))?;
        self.storage
            .put(&meta_key(&rec.summary.id), body, JSON, NO_CACHE)
            .await?;
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<TrackSummary>> {
        Ok(self.load().await?.tracks)
    }

    pub async fn find_by_source(&self, source_sha256: &str) -> Result<Option<TrackSummary>> {
        Ok(self
            .load()
            .await?
            .tracks
            .into_iter()
            .find(|t| t.source_sha256 == source_sha256))
    }

    pub async fn detail(&self, id: &str) -> Result<TrackRecord> {
        let bytes = self
            .storage
            .get(&meta_key(id))
            .await?
            .ok_or_else(|| LibraryError::NotFound(format!("track {id} not found")))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| LibraryError::Storage(format!("meta/{id}.json: {e}")))
    }

    pub async fn add(&self, p: ProcessedTrack) -> Result<AddOutcome> {
        let _guard = self.lock.lock().await;
        let cat = self.load().await?;
        if let Some(t) = cat
            .tracks
            .iter()
            .find(|t| t.source_sha256 == p.source_sha256 || t.id == p.id)
        {
            return Ok(AddOutcome::Duplicate(t.clone()));
        }

        let audio_key = format!("audio/{}.mp3", p.id);
        let cover_key = p
            .cover
            .as_ref()
            .map(|c| format!("cover/{}.{}", sha256_hex(&c.bytes), c.ext));
        let ts = now();
        let summary = TrackSummary {
            id: p.id.clone(),
            source_sha256: p.source_sha256.clone(),
            audio_url: self.url(&audio_key),
            audio_key: audio_key.clone(),
            cover_url: cover_key.as_deref().map(|k| self.url(k)),
            cover_key: cover_key.clone(),
            size_bytes: p.served.len() as u64,
            codec: p.codec.to_string(),
            bitrate: p.bitrate_kbps,
            vbr: p.vbr,
            sample_rate: p.sample_rate,
            channels: p.channels,
            duration_ms: p.duration_ms,
            title: p.tags.title.clone(),
            artist: p.tags.artist.clone(),
            album: p.tags.album.clone(),
            album_artist: p.tags.album_artist.clone(),
            date: p.tags.date.clone(),
            genre: p.tags.genre.clone(),
            track: p.tags.track.clone(),
            disc: p.tags.disc.clone(),
            integrated_lufs: p.analysis.integrated_lufs,
            true_peak_dbtp: p.analysis.true_peak_dbtp,
            lra: p.analysis.lra,
            effective_bandwidth_hz: p.analysis.effective_bandwidth_hz,
            quality_hint: p.analysis.quality_hint.map(str::to_string),
            trim_start: p.gapless.as_ref().map(|g| g.trim_start),
            trim_end: p.gapless.as_ref().map(|g| g.trim_end),
            total_samples: p.gapless.as_ref().map(|g| g.total_samples),
            analysis_version: ANALYSIS_VERSION,
            created_at: ts.clone(),
            updated_at: ts,
        };
        let record = TrackRecord {
            summary: summary.clone(),
            waveform: p.analysis.waveform.clone(),
        };

        // The audio put is the first write: if it fails, nothing needs rolling back.
        self.storage
            .put(&audio_key, p.served, "audio/mpeg", IMMUTABLE)
            .await?;

        let rest: Result<()> = async {
            if let (Some(c), Some(k)) = (&p.cover, &cover_key) {
                self.storage
                    .put(k, c.bytes.clone(), c.mime, IMMUTABLE)
                    .await?;
            }
            self.put_meta(&record).await?;
            let mut next = cat.clone();
            next.tracks.push(summary.clone());
            self.save(next).await
        }
        .await;

        if let Err(e) = rest {
            let _ = self.storage.delete(&audio_key).await;
            let _ = self.storage.delete(&meta_key(&p.id)).await;
            if let Some(k) = &cover_key
                && !cover_refs(&cat.tracks, k)
            {
                let _ = self.storage.delete(k).await;
            }
            return Err(e);
        }
        Ok(AddOutcome::Added(summary))
    }

    pub async fn update(
        &self,
        id: &str,
        edit: TagEdit,
        new_cover: Option<Vec<u8>>,
    ) -> Result<TrackSummary> {
        let title = edit.title.trim().to_string();
        if title.is_empty() {
            return Err(LibraryError::Validation("title must not be empty".into()));
        }
        let cover = match new_cover {
            None => None,
            Some(bytes) => {
                let (ext, mime) = sniff_image(&bytes).ok_or_else(|| {
                    LibraryError::Validation("cover is not a JPEG or PNG image".into())
                })?;
                Some((format!("cover/{}.{}", sha256_hex(&bytes), ext), bytes, mime))
            }
        };

        let _guard = self.lock.lock().await;
        let mut cat = self.load().await?;
        let idx = cat
            .tracks
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| LibraryError::NotFound(format!("track {id} not found")))?;
        let waveform = self.detail(id).await?.waveform;

        let old_cover = cat.tracks[idx].cover_key.clone();
        let mut t = cat.tracks[idx].clone();
        t.title = title;
        t.artist = clean(edit.artist);
        t.album = clean(edit.album);
        t.album_artist = clean(edit.album_artist);
        t.date = clean(edit.date);
        t.genre = clean(edit.genre);
        t.track = clean(edit.track);
        t.disc = clean(edit.disc);
        t.updated_at = now();
        if let Some((key, bytes, mime)) = cover {
            self.storage.put(&key, bytes, mime, IMMUTABLE).await?;
            t.cover_url = Some(self.url(&key));
            t.cover_key = Some(key);
        }

        self.put_meta(&TrackRecord {
            summary: t.clone(),
            waveform,
        })
        .await?;
        cat.tracks[idx] = t.clone();
        self.save(cat.clone()).await?;

        if let Some(old) = old_cover
            && t.cover_key.as_deref() != Some(old.as_str())
            && !cover_refs(&cat.tracks, &old)
        {
            let _ = self.storage.delete(&old).await;
        }
        Ok(t)
    }

    pub async fn delete(&self, ids: &[String]) -> Result<()> {
        let _guard = self.lock.lock().await;
        let mut cat = self.load().await?;
        let (removed, kept): (Vec<_>, Vec<_>) = cat
            .tracks
            .into_iter()
            .partition(|t| ids.contains(&t.id));
        if removed.is_empty() {
            return Ok(());
        }
        cat.tracks = kept;
        self.save(cat.clone()).await?;

        // Catalog is saved; orphaned objects are harmless, so errors are ignored.
        let mut seen = HashSet::new();
        for t in &removed {
            let _ = self.storage.delete(&t.audio_key).await;
            let _ = self.storage.delete(&meta_key(&t.id)).await;
            if let Some(k) = &t.cover_key
                && seen.insert(k.clone())
                && !cover_refs(&cat.tracks, k)
            {
                let _ = self.storage.delete(k).await;
            }
        }
        Ok(())
    }
}
