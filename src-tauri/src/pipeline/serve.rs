use std::sync::atomic::AtomicBool;

use super::analyze::analyze;
use super::lame::{first_frame, read_gapless, Gapless};
use super::REMUX_COVER_BYTES;
use crate::error::{ErrorCode, PipelineError};

/// Strips a trailing APEv2 tag ending at `end`, never cutting before `start`.
fn strip_ape(b: &[u8], start: usize, end: usize) -> usize {
    let Some(foot) = end.checked_sub(32).filter(|&f| f >= start) else {
        return end;
    };
    if &b[foot..foot + 8] != b"APETAGEX" {
        return end;
    }
    let size = u32::from_le_bytes([b[foot + 12], b[foot + 13], b[foot + 14], b[foot + 15]]) as usize;
    let flags = u32::from_le_bytes([b[foot + 20], b[foot + 21], b[foot + 22], b[foot + 23]]);
    let header = if flags & 0x8000_0000 != 0 { 32 } else { 0 };
    // `size` includes the footer but not the header.
    let tag_start = (foot + 32)
        .checked_sub(size)
        .and_then(|s| s.checked_sub(header));
    match tag_start {
        Some(s) if size >= 32 && s >= start => s,
        _ => end,
    }
}

/// Byte range `[start, end)` of the MPEG frames, or None if not an MP3.
fn audio_range(b: &[u8]) -> Option<(usize, usize)> {
    let start = first_frame(b)?.offset;
    let mut end = b.len();
    if end >= start + 128 && &b[end - 128..end - 125] == b"TAG" {
        end -= 128;
    }
    end = strip_ape(b, start, end);
    Some((start, end))
}

pub fn strip_tags(b: &[u8]) -> Vec<u8> {
    audio_range(b).map_or_else(Vec::new, |(s, e)| b[s..e].to_vec())
}

pub fn audio_payload_len(b: &[u8]) -> usize {
    audio_range(b).map_or(0, |(s, e)| e - s)
}

/// Builds the file to serve: the source bytes, or (cover > `REMUX_COVER_BYTES`)
/// the source with tags stripped, verified to decode to identical PCM with the
/// same gapless info. Returns `(served, remuxed)`.
pub fn build_served(
    source: &[u8],
    cover_len: Option<usize>,
    source_pcm: &[u8; 32],
    gapless: &Option<Gapless>,
    cancel: &AtomicBool,
) -> Result<(Vec<u8>, bool), PipelineError> {
    if !cover_len.is_some_and(|n| n > REMUX_COVER_BYTES) {
        return Ok((source.to_vec(), false));
    }
    let served = strip_tags(source);
    let fail = |what: &str| PipelineError::new(ErrorCode::Internal, format!("remux verification failed: {what}"));
    let re = analyze(&served, cancel)?;
    if &re.pcm_sha256 != source_pcm {
        return Err(fail("decoded PCM differs"));
    }
    if &read_gapless(&served) != gapless {
        return Err(fail("gapless info differs"));
    }
    Ok((served, true))
}
