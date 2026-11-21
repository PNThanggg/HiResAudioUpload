mod common;

use common::fixture;
use hires_upload_lib::error::ErrorCode;
use hires_upload_lib::pipeline::lame::{first_frame, read_gapless};
use hires_upload_lib::pipeline::{process, process_with_timeout, sha256_hex};
use std::sync::atomic::AtomicBool;

fn run(name: &str) -> (Vec<u8>, hires_upload_lib::pipeline::ProcessedTrack) {
    let src = fixture(name);
    let t = process(src.clone(), name, &AtomicBool::new(false)).unwrap();
    (src, t)
}

#[test]
fn byte_original_when_small_cover() {
    let (src, t) = run("cover_small");
    assert!(!t.remuxed);
    assert_eq!(t.served, src);
    assert_eq!(t.id, t.source_sha256);
    assert_eq!(t.id, sha256_hex(&src));
    assert_eq!(t.cover.unwrap().ext, "png");
}

#[test]
fn remux_when_big_cover() {
    let (src, t) = run("cover_big");
    assert!(t.remuxed);
    assert_ne!(t.id, t.source_sha256);
    assert!(t.served.len() < src.len());
    assert_eq!(read_gapless(&t.served), t.gapless);
}

#[test]
fn cbr_and_vbr_bitrate() {
    let (_, cbr) = run("pink_320");
    assert_eq!(cbr.bitrate_kbps, 320);
    assert!(!cbr.vbr);
    let (_, vbr) = run("pink_v2");
    assert!(vbr.vbr);
    assert!((90..=130).contains(&vbr.bitrate_kbps), "{}", vbr.bitrate_kbps);
}

#[test]
fn vbr_without_lame_id_keeps_vbr() {
    let mut src = fixture("pink_v2");
    let f = first_frame(&src).unwrap();
    let tag = f.offset + 4 + f.side_info_len;
    assert_eq!(&src[tag..tag + 4], b"Xing");
    let flags = u32::from_be_bytes(src[tag + 4..tag + 8].try_into().unwrap());
    let mut lame = tag + 8;
    for (bit, len) in [(1, 4), (2, 4), (4, 100), (8, 4)] {
        if flags & bit != 0 {
            lame += len;
        }
    }
    src[lame..lame + 9].fill(0); // encoder id no longer plausible
    assert!(read_gapless(&src).is_none());
    let t = process(src, "pink_v2_nolame", &AtomicBool::new(false)).unwrap();
    assert!(t.vbr);
    assert!(t.gapless.is_none());
    assert!((90..=130).contains(&t.bitrate_kbps), "{}", t.bitrate_kbps);
}

#[test]
fn duration_from_gapless() {
    assert_eq!(run("pink_320").1.duration_ms, 30000);
}

#[tokio::test]
async fn missing_file_is_internal() {
    let err = process_with_timeout("does/not/exist.mp3".into()).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Internal);
}

/// Perf check: a 4-minute 320k stereo MP3 must process in < 4 s (release build).
/// `FFMPEG=/path/to/ffmpeg cargo test --release --test pipeline perf_4min -- --ignored --nocapture`
#[test]
#[ignore]
fn perf_4min() {
    let path = std::env::temp_dir().join("hires_perf_4min.mp3");
    if !path.exists() {
        let tmp = std::env::temp_dir().join("hires_perf_4min.tmp.mp3");
        let ffmpeg = std::env::var("FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
        let status = std::process::Command::new(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg("anoisesrc=color=pink:sample_rate=44100:amplitude=0.1:duration=240:seed=7")
            .args(["-ac", "2", "-c:a", "libmp3lame", "-b:a", "320k"])
            .arg(&tmp)
            .status();
        if !matches!(&status, Ok(s) if s.success()) {
            let _ = std::fs::remove_file(&tmp);
            panic!("ffmpeg failed (set FFMPEG): {status:?}");
        }
        std::fs::rename(&tmp, &path).unwrap();
    }
    let src = std::fs::read(&path).unwrap();
    let start = std::time::Instant::now();
    let t = process(src, "perf_4min", &AtomicBool::new(false)).unwrap();
    let elapsed = start.elapsed();
    println!("perf_4min: process() took {elapsed:?} (duration {} ms)", t.duration_ms);
    assert!(elapsed.as_secs_f64() < 4.0, "too slow: {elapsed:?}");
}
