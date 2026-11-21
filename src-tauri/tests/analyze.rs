mod common;

use common::{fixture, oracle};
use hires_upload_lib::error::ErrorCode;
use hires_upload_lib::pipeline::analyze::analyze;
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
