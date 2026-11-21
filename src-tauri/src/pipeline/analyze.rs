//! Single decode pass feeding loudness, bandwidth, waveform and PCM hash (LC §0.4, §5).

use crate::error::{ErrorCode, PipelineError};
use crate::pipeline::bandwidth::{Spectrum, quality_hint};
use crate::pipeline::waveform::Waveform;
use ebur128::{EbuR128, Mode};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, Clone)]
pub struct Analysis {
    pub decoded_frames: u64,
    pub integrated_lufs: f64,
    pub true_peak_dbtp: f64,
    pub lra: f64,
    pub effective_bandwidth_hz: Option<u32>,
    pub quality_hint: Option<&'static str>,
    pub waveform: Vec<f32>,
    pub pcm_sha256: [u8; 32],
    /// Packets skipped because symphonia reported a `DecodeError` for them.
    pub bad_packets: u32,
}

/// Rejects the decode when nothing decoded or more than 1 % of packets were bad.
pub fn check_bad_packets(
    decoded_frames: u64,
    bad_packets: u32,
    total_packets: u32,
) -> Result<(), PipelineError> {
    if decoded_frames == 0 || u64::from(bad_packets) * 100 > u64::from(total_packets) {
        return Err(PipelineError::new(
            ErrorCode::InvalidFormat,
            format!("decode: {bad_packets} of {total_packets} packets failed to decode"),
        ));
    }
    Ok(())
}

fn invalid(e: impl std::fmt::Display) -> PipelineError {
    PipelineError::new(ErrorCode::InvalidFormat, format!("decode: {e}"))
}

fn internal(e: impl std::fmt::Display) -> PipelineError {
    PipelineError::new(ErrorCode::Internal, format!("loudness: {e}"))
}

pub fn analyze(bytes: &[u8], cancel: &AtomicBool) -> Result<Analysis, PipelineError> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            Hint::new().with_extension("mp3"),
            mss,
            &FormatOptions { enable_gapless: true, ..Default::default() },
            &MetadataOptions::default(),
        )
        .map_err(invalid)?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| invalid("no audio track"))?;
    let track_id = track.id;
    let params = track.codec_params.clone();
    let sample_rate = params.sample_rate.ok_or_else(|| invalid("unknown sample rate"))?;
    let channels = params.channels.ok_or_else(|| invalid("unknown channel layout"))?.count();
    let mut decoder = symphonia::default::get_codecs().make(&params, &DecoderOptions::default()).map_err(invalid)?;

    let mut meter = EbuR128::new(channels as u32, sample_rate, Mode::I | Mode::LRA | Mode::TRUE_PEAK)
        .map_err(internal)?;
    let mut spectrum = Spectrum::new(sample_rate);
    let mut waveform = Waveform::new(channels);
    let mut hasher = Sha256::new();
    let mut le_bytes: Vec<u8> = Vec::new();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut decoded_frames: u64 = 0;
    let mut total_packets: u32 = 0;
    let mut bad_packets: u32 = 0;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(PipelineError::new(ErrorCode::Timeout, "analysis cancelled"));
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(invalid(e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        total_packets += 1;
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => {
                bad_packets += 1;
                continue;
            }
            Err(e) => return Err(invalid(e)),
        };
        if decoded.spec().channels.count() != channels || decoded.spec().rate != sample_rate {
            return Err(invalid("stream format changes mid-file"));
        }
        if decoded.frames() == 0 {
            continue;
        }
        let buf = match &mut sample_buf {
            Some(b) if b.capacity() >= decoded.frames() * channels => b,
            slot => slot.insert(SampleBuffer::new(decoded.capacity() as u64, *decoded.spec())),
        };
        buf.copy_interleaved_ref(decoded);
        let samples = buf.samples();

        decoded_frames += (samples.len() / channels) as u64;
        meter.add_frames_f32(samples).map_err(internal)?;
        spectrum.push(samples, channels);
        waveform.push(samples);
        le_bytes.clear();
        le_bytes.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
        hasher.update(&le_bytes);
    }

    check_bad_packets(decoded_frames, bad_packets, total_packets)?;
    if decoded_frames < u64::from(sample_rate) {
        return Err(PipelineError::new(ErrorCode::TooShort, "decoded audio is shorter than 1 s"));
    }
    let integrated_lufs = meter.loudness_global().map_err(internal)?;
    if integrated_lufs.is_nan() || integrated_lufs < -70.0 {
        return Err(PipelineError::new(ErrorCode::Silent, "integrated loudness below -70 LUFS"));
    }
    let lra = meter.loudness_range().map_err(internal)?;
    let mut peak = 0.0f64;
    for ch in 0..channels as u32 {
        peak = peak.max(meter.true_peak(ch).map_err(internal)?);
    }
    let effective_bandwidth_hz = spectrum.finish();

    Ok(Analysis {
        decoded_frames,
        integrated_lufs,
        true_peak_dbtp: 20.0 * peak.log10(),
        lra,
        effective_bandwidth_hz,
        quality_hint: effective_bandwidth_hz.map(|bw| quality_hint(bw, sample_rate)),
        waveform: waveform.finish(),
        pcm_sha256: hasher.finalize().into(),
        bad_packets,
    })
}
