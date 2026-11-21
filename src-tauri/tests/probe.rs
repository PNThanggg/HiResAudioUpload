mod common;

use common::fixture;
use hires_upload_lib::error::ErrorCode;
use hires_upload_lib::pipeline::probe::{check_size, probe, sniff_image, MAX_BYTES};

#[test]
fn fake_mp3_is_invalid() {
    assert_eq!(probe(&fixture("fake"), "fake").unwrap_err().code, ErrorCode::InvalidFormat);
}

#[test]
fn reads_unicode_tags_and_big_cover() {
    let p = probe(&fixture("cover_big"), "cover_big").unwrap();
    assert_eq!(p.tags.title, "Bài hát thử");
    assert_eq!(p.tags.artist.as_deref(), Some("Nghệ sĩ"));
    assert_eq!(p.tags.album.as_deref(), Some("Album thử"));
    let c = p.cover.unwrap();
    assert_eq!(c.ext, "jpg");
    assert!(c.bytes.len() > 200 * 1024);
}

#[test]
fn small_png_cover() {
    assert_eq!(probe(&fixture("cover_small"), "x").unwrap().cover.unwrap().ext, "png");
}

#[test]
fn title_falls_back_to_file_stem_unicode() {
    let p = probe(&fixture("noxing_notag"), "Nhạc Trịnh – Diễm xưa").unwrap();
    assert_eq!(p.tags.title, "Nhạc Trịnh – Diễm xưa");
    assert!(p.gapless.is_none());
    assert!(p.cover.is_none());
}

#[test]
fn size_limit() {
    assert!(check_size(MAX_BYTES).is_ok());
    assert_eq!(check_size(MAX_BYTES + 1).unwrap_err().code, ErrorCode::TooLarge);
}

#[test]
fn sniff_and_serialize() {
    assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0]), Some(("jpg", "image/jpeg")));
    assert_eq!(sniff_image(&[0x89, 0x50, 0x4E, 0x47]), Some(("png", "image/png")));
    assert_eq!(sniff_image(b"GIF8"), None);
    assert_eq!(serde_json::to_string(&ErrorCode::InvalidFormat).unwrap(), "\"INVALID_FORMAT\"");
}
