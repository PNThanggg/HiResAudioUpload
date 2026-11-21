use std::io::Cursor;

use lofty::file::TaggedFileExt;
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};

use crate::error::{ErrorCode, PipelineError};
use crate::pipeline::lame::{first_frame, read_gapless, FrameHeader, Gapless};

pub const MAX_BYTES: u64 = 200 * 1024 * 1024;
const MAX_SECONDS: u64 = 7200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tags {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub track: Option<String>,
    pub disc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub bytes: Vec<u8>,
    pub ext: &'static str,
    pub mime: &'static str,
}

#[derive(Debug, Clone)]
pub struct Probed {
    pub header: FrameHeader,
    pub gapless: Option<Gapless>,
    pub tags: Tags,
    pub cover: Option<Cover>,
}

pub fn check_size(len: u64) -> Result<(), PipelineError> {
    if len > MAX_BYTES {
        return Err(PipelineError::new(
            ErrorCode::TooLarge,
            format!("file is {len} bytes, limit is {MAX_BYTES}"),
        ));
    }
    Ok(())
}

pub fn sniff_image(b: &[u8]) -> Option<(&'static str, &'static str)> {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("jpg", "image/jpeg"))
    } else if b.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some(("png", "image/png"))
    } else {
        None
    }
}

fn text(tag: &Tag, key: ItemKey) -> Option<String> {
    tag.get_string(&key)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn read_tag(bytes: &[u8]) -> Option<Tag> {
    let tagged = Probe::new(Cursor::new(bytes))
        .guess_file_type()
        .ok()?
        .read()
        .ok()?;
    tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .cloned()
}

pub fn probe(bytes: &[u8], file_stem: &str) -> Result<Probed, PipelineError> {
    let header = first_frame(bytes)
        .ok_or_else(|| PipelineError::new(ErrorCode::InvalidFormat, "not an MPEG layer III file"))?;
    if header.channels != 1 && header.channels != 2 {
        return Err(PipelineError::new(ErrorCode::InvalidFormat, "unsupported channel count"));
    }
    let sr_ok = if header.mpeg1 {
        matches!(header.sample_rate, 32000 | 44100 | 48000)
    } else {
        matches!(header.sample_rate, 8000 | 11025 | 12000 | 16000 | 22050 | 24000)
    };
    if !sr_ok {
        return Err(PipelineError::new(
            ErrorCode::InvalidFormat,
            format!("unsupported sample rate {}", header.sample_rate),
        ));
    }
    let gapless = read_gapless(bytes);
    if let Some(g) = &gapless
        && g.total_samples / u64::from(header.sample_rate) > MAX_SECONDS
    {
        return Err(PipelineError::new(ErrorCode::TooLarge, "duration exceeds 2 hours"));
    }

    let tag = read_tag(bytes);
    let mut tags = Tags {
        title: file_stem.to_owned(),
        artist: None,
        album: None,
        album_artist: None,
        date: None,
        genre: None,
        track: None,
        disc: None,
    };
    let mut cover = None;
    if let Some(t) = &tag {
        if let Some(title) = text(t, ItemKey::TrackTitle) {
            tags.title = title;
        }
        tags.artist = text(t, ItemKey::TrackArtist);
        tags.album = text(t, ItemKey::AlbumTitle);
        tags.album_artist = text(t, ItemKey::AlbumArtist);
        tags.date = text(t, ItemKey::RecordingDate).or_else(|| text(t, ItemKey::Year));
        tags.genre = text(t, ItemKey::Genre);
        tags.track = text(t, ItemKey::TrackNumber);
        tags.disc = text(t, ItemKey::DiscNumber);

        let pics = t.pictures();
        let pic = pics
            .iter()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or_else(|| pics.first());
        if let Some(p) = pic
            && let Some((ext, mime)) = sniff_image(p.data())
        {
            cover = Some(Cover { bytes: p.data().to_vec(), ext, mime });
        }
    }
    Ok(Probed { header, gapless, tags, cover })
}
