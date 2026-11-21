mod common;

use common::fixture;
use hires_upload_lib::pipeline::lame::{first_frame, id3v2_len, read_gapless, read_xing};

#[test]
fn syncsafe_and_footer() {
    let mut b = vec![b'I', b'D', b'3', 4, 0, 0x10, 0, 0, 0x02, 0x01]; // size = 2*128+1 = 257, footer flag
    b.resize(10 + 257 + 10, 0);
    assert_eq!(id3v2_len(&b), 277);
}

#[test]
fn gapless_matches_lc_example() {
    for f in ["pink_320", "pink_128to320", "pink_loud", "pink_v2", "pink_128"] {
        let g = read_gapless(&fixture(f)).unwrap();
        assert_eq!(g.trim_start, 1105, "{f}");
        assert_eq!(g.total_samples, 1_323_000, "{f}");
        assert_eq!(g.trim_end, g.padding.saturating_sub(529));
    }
    assert!(!read_gapless(&fixture("pink_320")).unwrap().vbr);
    assert!(read_gapless(&fixture("pink_v2")).unwrap().vbr);
}

#[test]
fn no_xing_is_none() {
    assert!(read_gapless(&fixture("noxing_notag")).is_none());
}

#[test]
fn finds_frame_after_padding_and_footer() {
    // ID3 tag (footer flag) + 3000 zero bytes, then the frames of pink_128
    let src = fixture("pink_128");
    let audio = &src[id3v2_len(&src)..];
    let mut b = vec![b'I', b'D', b'3', 4, 0, 0x10, 0, 0, 0, 10];
    b.resize(30, 0); // 10 hdr + 10 body + 10 footer
    b.extend(std::iter::repeat_n(0u8, 3000));
    b.extend_from_slice(audio);
    assert_eq!(read_gapless(&b), read_gapless(&src));
    assert_eq!(first_frame(&b).unwrap().bitrate_kbps, 128);
}

#[test]
fn garbage_is_none() {
    assert!(first_frame(&[0xFF; 64]).is_none());
    assert!(read_gapless(b"hello").is_none());
}

/// One MPEG-1 layer III 128 kbps / 44.1 kHz stereo frame (417 bytes) carrying a Xing tag
/// (frames field only) followed by `lame_id` as the 9-byte encoder id.
fn xing_frame(lame_id: &[u8; 9]) -> Vec<u8> {
    let mut b = vec![0u8; 417];
    b[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
    let tag = 4 + 32;
    b[tag..tag + 4].copy_from_slice(b"Xing");
    b[tag + 4..tag + 8].copy_from_slice(&1u32.to_be_bytes());
    b[tag + 8..tag + 12].copy_from_slice(&100u32.to_be_bytes());
    b[tag + 12..tag + 21].copy_from_slice(lame_id);
    b
}

#[test]
fn xing_without_lame_id_is_none() {
    assert!(read_gapless(&xing_frame(&[0; 9])).is_none());
    assert!(read_gapless(&xing_frame(b"\x01AME3.100")).is_none());
    assert!(read_gapless(&xing_frame(b"3.100LAME")).is_none());
    let g = read_gapless(&xing_frame(b"LAME3.100")).unwrap();
    assert_eq!(g.encoder, "LAME3.100");
    assert_eq!(read_gapless(&xing_frame(b"L3.99r\0\0\0")).unwrap().encoder, "L3.99r");
}

#[test]
fn xing_survives_missing_lame_id() {
    let x = read_xing(&xing_frame(&[0; 9])).unwrap();
    assert!(x.vbr);
    assert_eq!(x.frames, Some(100));
    let mut info = xing_frame(&[0; 9]);
    info[36..40].copy_from_slice(b"Info");
    assert!(!read_xing(&info).unwrap().vbr);
    assert!(read_xing(&fixture("noxing_notag")).is_none());
    assert!(read_xing(&fixture("pink_v2")).unwrap().vbr);
}
