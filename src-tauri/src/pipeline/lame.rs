//! MPEG audio frame header and LAME/Xing gapless tag parsing (logic-core section 4).

const BITRATES_V1: [u32; 14] = [32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const BITRATES_V2: [u32; 14] = [8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

/// Decoder delay added by every MP3 decoder, in samples per channel.
const DECODER_DELAY: u32 = 529;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    pub offset: usize,
    pub mpeg1: bool,
    pub channels: u8,
    pub sample_rate: u32,
    pub bitrate_kbps: u32,
    pub samples_per_frame: u32,
    pub side_info_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gapless {
    pub encoder: String,
    pub vbr: bool,
    pub frames: u32,
    pub delay: u32,
    pub padding: u32,
    pub trim_start: u32,
    pub trim_end: u32,
    pub total_samples: u64,
}

/// Length of the leading ID3v2 tag (header + body + optional footer), or 0 if absent.
pub fn id3v2_len(b: &[u8]) -> usize {
    if b.len() < 10 || &b[..3] != b"ID3" {
        return 0;
    }
    let size = b[6..10]
        .iter()
        .fold(0usize, |acc, &x| (acc << 7) | (x & 0x7F) as usize);
    let footer = if b[5] & 0x10 != 0 { 10 } else { 0 };
    10 + size + footer
}

/// Parses a layer III header at `pos`; returns the header and its frame length in bytes.
fn parse_header(b: &[u8], pos: usize) -> Option<(FrameHeader, usize)> {
    let h = b.get(pos..pos + 4)?;
    if h[0] != 0xFF || h[1] & 0xE0 != 0xE0 {
        return None;
    }
    let mpeg1 = match (h[1] >> 3) & 3 {
        3 => true,
        2 | 0 => false,
        _ => return None,
    };
    if (h[1] >> 1) & 3 != 1 {
        return None; // not layer III
    }
    let br_idx = (h[2] >> 4) as usize;
    if br_idx == 0 || br_idx == 15 {
        return None;
    }
    let sr_idx = ((h[2] >> 2) & 3) as usize;
    if sr_idx == 3 {
        return None;
    }
    let sample_rate = match (h[1] >> 3) & 3 {
        3 => [44100, 48000, 32000][sr_idx],
        2 => [22050, 24000, 16000][sr_idx],
        _ => [11025, 12000, 8000][sr_idx],
    };
    let bitrate_kbps = if mpeg1 { BITRATES_V1 } else { BITRATES_V2 }[br_idx - 1];
    let padding = ((h[2] >> 1) & 1) as u32;
    let mono = h[3] >> 6 == 3;
    let (coef, samples_per_frame) = if mpeg1 { (144_000, 1152) } else { (72_000, 576) };
    let len = (coef * bitrate_kbps / sample_rate + padding) as usize;
    let header = FrameHeader {
        offset: pos,
        mpeg1,
        channels: if mono { 1 } else { 2 },
        sample_rate,
        bitrate_kbps,
        samples_per_frame,
        side_info_len: match (mpeg1, mono) {
            (true, true) => 17,
            (true, false) => 32,
            (false, true) => 9,
            (false, false) => 17,
        },
    };
    Some((header, len))
}

/// Finds the first plausible MPEG layer III frame after the ID3v2 tag.
/// A candidate must be followed by another valid header (or the end of the data).
pub fn first_frame(b: &[u8]) -> Option<FrameHeader> {
    let start = id3v2_len(b);
    (start..b.len().saturating_sub(3)).find_map(|pos| {
        let (header, len) = parse_header(b, pos)?;
        let next = pos + len;
        (next == b.len() || parse_header(b, next).is_some()).then_some(header)
    })
}

fn be32(b: &[u8], pos: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(pos..pos + 4)?.try_into().ok()?))
}

/// Xing/Info header of the first frame, independent of any LAME extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xing {
    /// `true` for a "Xing" tag (VBR), `false` for "Info" (CBR).
    pub vbr: bool,
    /// Frame count, when the frames flag is set.
    pub frames: Option<u32>,
}

/// Parses the Xing/Info header; also returns the first frame and the offset just past
/// the optional Xing fields, where a LAME extension would start.
fn xing_at(b: &[u8]) -> Option<(FrameHeader, Xing, usize)> {
    let f = first_frame(b)?;
    let tag = f.offset + 4 + f.side_info_len;
    let vbr = match b.get(tag..tag + 4)? {
        b"Xing" => true,
        b"Info" => false,
        _ => return None,
    };
    let flags = be32(b, tag + 4)?;
    let mut pos = tag + 8;
    let mut frames = None;
    if flags & 1 != 0 {
        frames = Some(be32(b, pos)?);
        pos += 4;
    }
    if flags & 2 != 0 {
        pos += 4;
    }
    if flags & 4 != 0 {
        pos += 100;
    }
    if flags & 8 != 0 {
        pos += 4;
    }
    Some((f, Xing { vbr, frames }, pos))
}

/// Reads the Xing/Info header of the first frame (VBR flag + frame count).
pub fn read_xing(b: &[u8]) -> Option<Xing> {
    xing_at(b).map(|(_, x, _)| x)
}

/// Reads delay/padding from the Xing/Info + LAME tag of the first frame.
pub fn read_gapless(b: &[u8]) -> Option<Gapless> {
    let (f, Xing { vbr, frames }, pos) = xing_at(b)?;
    let frames = frames?;
    let lame = b.get(pos..pos + 24)?;
    // Only trust delay/padding behind a plausible encoder id ("LAME3.100", "Lavc58.13", "L3.99r").
    let id = &lame[..9];
    let id = &id[..id.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1)];
    if !id.first().is_some_and(u8::is_ascii_alphabetic)
        || !id.iter().all(|&c| (0x20..=0x7E).contains(&c))
    {
        return None;
    }
    let encoder = String::from_utf8_lossy(id).trim().to_string();
    let delay = ((lame[21] as u32) << 4) | (lame[22] >> 4) as u32;
    let padding = (((lame[22] & 0x0F) as u32) << 8) | lame[23] as u32;
    let total = frames as u64 * f.samples_per_frame as u64;
    Some(Gapless {
        encoder,
        vbr,
        frames,
        delay,
        padding,
        trim_start: delay + DECODER_DELAY,
        trim_end: padding.saturating_sub(DECODER_DELAY),
        total_samples: total.saturating_sub(delay as u64 + padding as u64),
    })
}
