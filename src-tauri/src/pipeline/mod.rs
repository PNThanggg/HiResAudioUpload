pub mod analyze;
pub mod bandwidth;
pub mod lame;
pub mod probe;
pub mod serve;
pub mod waveform;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::error::{ErrorCode, PipelineError};
use analyze::Analysis;
use lame::Gapless;
use probe::{Cover, Tags};

pub const REMUX_COVER_BYTES: usize = 200 * 1024;
pub const TIMEOUT: Duration = Duration::from_secs(300);
pub const ANALYSIS_VERSION: u32 = 1;
const MAX_DURATION_MS: u64 = 7_200_000;

#[derive(Debug, Clone)]
pub struct ProcessedTrack {
    pub id: String,
    pub source_sha256: String,
    pub served: Vec<u8>,
    pub remuxed: bool,
    pub cover: Option<Cover>,
    pub tags: Tags,
    pub codec: &'static str,
    pub bitrate_kbps: u32,
    pub vbr: bool,
    pub sample_rate: u32,
    pub channels: u8,
    pub duration_ms: u64,
    pub analysis: Analysis,
    pub gapless: Option<Gapless>,
}

pub fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

pub fn process(
    source: Vec<u8>,
    file_stem: &str,
    cancel: &AtomicBool,
) -> Result<ProcessedTrack, PipelineError> {
    let source_sha256 = sha256_hex(&source);
    let probed = probe::probe(&source, file_stem)?;
    let analysis = analyze::analyze(&source, cancel)?;
    let (served, remuxed) = serve::build_served(
        &source,
        probed.cover.as_ref().map(|c| c.bytes.len()),
        &analysis.pcm_sha256,
        &probed.gapless,
        cancel,
    )?;
    let id = sha256_hex(&served);

    let sr = probed.header.sample_rate as u64;
    // Without gapless data, count each skipped (undecodable) packet as one full frame.
    let skipped = u64::from(analysis.bad_packets) * u64::from(probed.header.samples_per_frame);
    let frames = probed
        .gapless
        .as_ref()
        .map_or(analysis.decoded_frames + skipped, |g| g.total_samples);
    let duration_ms = frames * 1000 / sr;
    if duration_ms > MAX_DURATION_MS {
        return Err(PipelineError::new(ErrorCode::TooLarge, "duration exceeds 2 hours"));
    }

    // A Xing header marks VBR even when no trustworthy LAME tag follows it.
    let vbr = lame::read_xing(&source).is_some_and(|x| x.vbr);
    let bitrate_kbps = if vbr && duration_ms > 0 {
        (serve::audio_payload_len(&source) as f64 * 8.0 / (duration_ms as f64 / 1000.0) / 1000.0)
            .round() as u32
    } else {
        probed.header.bitrate_kbps
    };

    Ok(ProcessedTrack {
        id,
        source_sha256,
        served,
        remuxed,
        cover: probed.cover,
        tags: probed.tags,
        codec: "mp3",
        bitrate_kbps,
        vbr,
        sample_rate: probed.header.sample_rate,
        channels: probed.header.channels,
        duration_ms,
        analysis,
        gapless: probed.gapless,
    })
}

fn internal(e: impl std::fmt::Display) -> PipelineError {
    PipelineError::new(ErrorCode::Internal, e.to_string())
}

pub async fn process_with_timeout(path: PathBuf) -> Result<ProcessedTrack, PipelineError> {
    let len = std::fs::metadata(&path).map_err(internal)?.len();
    probe::check_size(len)?;
    let source = std::fs::read(&path).map_err(internal)?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    process_bytes_with_timeout(source, stem).await
}

pub async fn process_bytes_with_timeout(
    source: Vec<u8>,
    stem: String,
) -> Result<ProcessedTrack, PipelineError> {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let task = tokio::task::spawn_blocking(move || process(source, &stem, &flag));
    match tokio::time::timeout(TIMEOUT, task).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => Err(internal(e)),
        Err(_) => {
            cancel.store(true, Ordering::Relaxed);
            Err(PipelineError::new(ErrorCode::Timeout, "processing timed out"))
        }
    }
}
