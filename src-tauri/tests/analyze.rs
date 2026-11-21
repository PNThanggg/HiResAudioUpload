mod common;

use common::{fixture, oracle};
use hires_upload_lib::error::ErrorCode;
use hires_upload_lib::pipeline::analyze::{analyze, check_bad_packets};
use hires_upload_lib::pipeline::lame::first_frame;
use hires_upload_lib::pipeline::serve::strip_tags;
use std::sync::atomic::AtomicBool;

#[test]
fn matches_ffmpeg_oracle_and_hints() {
    let o = oracle();
    for (f, hint) in [
        ("pink_320", "full"),
        ("pink_128to320", "medium"),
        ("pink_loud", "full"),
        ("pink_v2", "high"),
        ("pink_128", "medium"),
    ] {
        let a = analyze(&fixture(f), &AtomicBool::new(false)).unwrap();
        assert!((a.integrated_lufs - o[f].lufs).abs() <= 0.3, "{f} lufs");
        assert!((a.true_peak_dbtp - o[f].true_peak_dbtp).abs() <= 0.3, "{f} tp");
        assert!((a.lra - o[f].lra).abs() <= 0.3, "{f} lra");
        assert_eq!(a.quality_hint, Some(hint), "{f} bw={:?}", a.effective_bandwidth_hz);
        assert_eq!(a.decoded_frames, 1_323_000, "{f} gapless trim");
        assert_eq!(a.waveform.len(), 800);
    }
    assert!(analyze(&fixture("pink_loud"), &AtomicBool::new(false)).unwrap().true_peak_dbtp > 0.0);
}

#[test]
fn silent_and_short_rejected() {
    assert_eq!(analyze(&fixture("silent"), &AtomicBool::new(false)).unwrap_err().code, ErrorCode::Silent);
    assert_eq!(analyze(&fixture("short"), &AtomicBool::new(false)).unwrap_err().code, ErrorCode::TooShort);
}

#[test]
fn cancel_gives_timeout() {
    assert_eq!(analyze(&fixture("pink_320"), &AtomicBool::new(true)).unwrap_err().code, ErrorCode::Timeout);
}

#[test]
fn pcm_hash_stable_across_strip() {
    let src = fixture("cover_big");
    let c = AtomicBool::new(false);
    assert_eq!(analyze(&src, &c).unwrap().pcm_sha256, analyze(&strip_tags(&src), &c).unwrap().pcm_sha256);
}

/// Offsets of the MPEG-1 layer III frames of a CBR/VBR file (starting at the Xing frame).
fn frame_offsets(b: &[u8]) -> Vec<usize> {
    const BR: [u32; 14] = [32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
    let first = first_frame(b).unwrap();
    assert!(first.mpeg1 && first.sample_rate == 44_100 && first.channels == 2);
    let mut out = Vec::new();
    let mut pos = first.offset;
    while pos + 4 <= b.len() && b[pos] == 0xFF && b[pos + 1] & 0xE0 == 0xE0 {
        out.push(pos);
        let br = BR[(b[pos + 2] >> 4) as usize - 1];
        let pad = ((b[pos + 2] >> 1) & 1) as u32;
        pos += (144_000 * br / 44_100 + pad) as usize;
    }
    out
}

/// Sets big_values of granule 0 / channel 0 to 511 (> 288), which symphonia rejects with a
/// `DecodeError` while the frame header (and so the demuxer) stays intact.
fn corrupt_side_info(b: &mut [u8], frame: usize) {
    assert_eq!(b[frame + 1] & 1, 1, "fixture frames carry no CRC");
    let side = frame + 4;
    b[side + 4] = 0xFF;
    b[side + 5] |= 0x80;
}

#[test]
fn single_bad_packet_is_skipped() {
    let mut b = fixture("pink_128");
    let frames = frame_offsets(&b);
    let clean = analyze(&b, &AtomicBool::new(false)).unwrap();
    assert_eq!(clean.bad_packets, 0);
    corrupt_side_info(&mut b, frames[frames.len() / 2]);
    let a = analyze(&b, &AtomicBool::new(false)).unwrap();
    assert_eq!(a.bad_packets, 1);
    assert!(a.decoded_frames < clean.decoded_frames);
}

#[test]
fn many_bad_packets_are_invalid_format() {
    let mut b = fixture("pink_128");
    let frames = frame_offsets(&b);
    // Every 50th frame: ~2 % of the packets.
    for &f in frames.iter().skip(10).step_by(50) {
        corrupt_side_info(&mut b, f);
    }
    let e = analyze(&b, &AtomicBool::new(false)).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidFormat);
    assert!(e.message.contains("packets failed to decode"), "{}", e.message);
}

#[test]
fn bad_packet_threshold() {
    assert!(check_bad_packets(1000, 0, 1000).is_ok());
    assert!(check_bad_packets(1000, 10, 1000).is_ok());
    let e = check_bad_packets(1000, 11, 1000).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidFormat);
    assert!(e.message.contains("11 of 1000"), "{}", e.message);
    assert_eq!(check_bad_packets(0, 0, 0).unwrap_err().code, ErrorCode::InvalidFormat);
}
