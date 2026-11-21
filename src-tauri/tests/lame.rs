mod common;

use common::fixture;
use hires_upload_lib::pipeline::lame::{first_frame, id3v2_len, read_gapless};

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
    b.extend(std::iter::repeat(0u8).take(3000));
    b.extend_from_slice(audio);
    assert_eq!(read_gapless(&b), read_gapless(&src));
    assert_eq!(first_frame(&b).unwrap().bitrate_kbps, 128);
}

#[test]
fn garbage_is_none() {
    assert!(first_frame(&[0xFF; 64]).is_none());
    assert!(read_gapless(b"hello").is_none());
}
