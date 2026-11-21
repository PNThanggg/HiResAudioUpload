mod common;

use common::fixture;
use hires_upload_lib::pipeline::lame::read_gapless;
use hires_upload_lib::pipeline::serve::{audio_payload_len, strip_tags};

#[test]
fn strip_removes_id3v2_and_cover() {
    let source = fixture("cover_big");
    let stripped = strip_tags(&source);
    assert_eq!(stripped[0], 0xFF);
    assert!(source.len() - stripped.len() > 200 * 1024);
    assert_eq!(read_gapless(&stripped), read_gapless(&source));
    assert_eq!(audio_payload_len(&source), stripped.len());
}

fn ape_block() -> Vec<u8> {
    let items = vec![b'x'; 24];
    let mut header = Vec::new();
    header.extend_from_slice(b"APETAGEX");
    header.extend_from_slice(&2000u32.to_le_bytes());
    header.extend_from_slice(&((items.len() + 32) as u32).to_le_bytes());
    header.extend_from_slice(&1u32.to_le_bytes());
    header.extend_from_slice(&0xA000_0000u32.to_le_bytes()); // has header + is header
    header.extend_from_slice(&[0; 8]);
    let mut footer = header.clone();
    footer[20..24].copy_from_slice(&0x8000_0000u32.to_le_bytes());
    let mut out = header;
    out.extend_from_slice(&items);
    out.extend_from_slice(&footer);
    out
}

fn id3v1() -> Vec<u8> {
    let mut t = vec![0u8; 128];
    t[..3].copy_from_slice(b"TAG");
    t
}

#[test]
fn strip_removes_id3v1_and_ape() {
    let frames = strip_tags(&fixture("pink_128"));
    let mut input = frames.clone();
    input.extend(ape_block());
    input.extend(id3v1());
    assert_eq!(strip_tags(&input), frames);
    assert_eq!(audio_payload_len(&input), frames.len());

    let mut ape_only = frames.clone();
    ape_only.extend(ape_block());
    assert_eq!(strip_tags(&ape_only), frames);

    let mut ape_then_v1_then_ape = frames.clone();
    ape_then_v1_then_ape.extend(ape_block());
    ape_then_v1_then_ape.extend(id3v1());
    assert_eq!(strip_tags(&ape_then_v1_then_ape), frames);
}

#[test]
fn strip_is_identity_on_bare_frames() {
    let once = strip_tags(&fixture("pink_128"));
    assert_eq!(strip_tags(&once), once);
}

#[test]
fn strip_non_mp3_is_empty() {
    assert!(strip_tags(&[1, 2, 3]).is_empty());
    assert_eq!(audio_payload_len(&[]), 0);
}
