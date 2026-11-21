//! Live end-to-end check against a real R2 bucket. Never runs by default.
//!
//! Credentials: env `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_BUCKET`,
//! `R2_PUBLIC_BASE_URL`; otherwise the app's settings file
//! (`%APPDATA%\audios.hi-res.upload\settings.json` on Windows).
//!
//!   cargo test --test e2e_live -- --ignored --nocapture
mod common;

use common::fixture;
use hires_upload_lib::catalog::{AddOutcome, Library, TagEdit, TrackSummary};
use hires_upload_lib::pipeline::probe::probe;
use hires_upload_lib::pipeline::process;
use hires_upload_lib::r2::R2Storage;
use hires_upload_lib::settings::{self, Settings};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

fn credentials() -> Option<Settings> {
    let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let from_env = (|| {
        Some(Settings {
            account_id: get("R2_ACCOUNT_ID")?,
            access_key_id: get("R2_ACCESS_KEY_ID")?,
            secret_access_key: get("R2_SECRET_ACCESS_KEY")?,
            bucket: get("R2_BUCKET")?,
            public_base_url: get("R2_PUBLIC_BASE_URL")?,
        })
    })();
    let s = match from_env {
        Some(s) => s,
        None => {
            let dir = std::env::var_os("APPDATA")
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))?;
            settings::load(&dir.join("audios.hi-res.upload").join("settings.json"))
        }
    };
    let s = s.normalize();
    s.is_complete().then_some(s)
}

async fn get(http: &reqwest::Client, url: &str, range: bool) -> reqwest::Response {
    let mut req = http.get(url);
    if range {
        req = req.header("Range", "bytes=0-1023");
    }
    req.send().await.unwrap_or_else(|e| panic!("GET {url}: {e}"))
}

/// Retries `check` for up to ~10 s (CDN may briefly serve stale objects).
async fn eventually<F, Fut>(what: &str, mut check: F) -> Result<(), String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let mut last = String::new();
    for _ in 0..10 {
        match check().await {
            Ok(()) => return Ok(()),
            Err(e) => last = e,
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(format!("{what}: {last}"))
}

async fn status_of(http: &reqwest::Client, url: &str) -> u16 {
    get(http, url, false).await.status().as_u16()
}

fn header(r: &reqwest::Response, k: &str) -> String {
    r.headers().get(k).and_then(|v| v.to_str().ok()).unwrap_or("").to_string()
}

#[tokio::test]
#[ignore]
async fn e2e_live() {
    let Some(s) = credentials() else {
        eprintln!("SKIP e2e_live: set R2_* env vars (incl. R2_PUBLIC_BASE_URL) or fill in the app Settings");
        return;
    };
    let base = s.public_base_url.clone();
    let lib = Library::new(Arc::new(R2Storage::new(&s).await), base.clone());
    let http = reqwest::Client::new();

    let before: Vec<String> = lib.list().await.unwrap().into_iter().map(|t| t.id).collect();
    println!("[a] pre-existing tracks in bucket: {}", before.len());

    let mut added: Vec<String> = Vec::new();
    let result = run(&lib, &http, &base, &before, &mut added).await;
    if !added.is_empty() {
        println!("[cleanup] deleting {} track(s) added by this test", added.len());
        if let Err(e) = lib.delete(&added).await {
            eprintln!("cleanup failed (delete manually): {e:?}");
        }
    }
    if let Err(e) = result {
        panic!("{e}");
    }
    println!("E2E OK");
}

async fn run(
    lib: &Library,
    http: &reqwest::Client,
    base: &str,
    before: &[String],
    added: &mut Vec<String>,
) -> Result<(), String> {
    // b. process + add
    let mut tracks: Vec<TrackSummary> = Vec::new();
    for name in ["cover_big", "pink_v2", "noxing_notag"] {
        let p = process(fixture(name), name, &AtomicBool::new(false)).map_err(|e| format!("{e:?}"))?;
        match lib.add(p).await.map_err(|e| format!("add {name}: {e:?}"))? {
            AddOutcome::Added(t) => {
                println!("[b] added {name} -> {}", t.id);
                added.push(t.id.clone());
                tracks.push(t);
            }
            AddOutcome::Duplicate(t) => {
                println!("[b] {name} already in bucket -> {} (will not be deleted)", t.id);
                tracks.push(t);
            }
        }
    }
    let cover_big = tracks[0].clone();

    // c. public HTTP checks
    let ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
    eventually("catalog.json lists the 3 ids without waveform", || async {
        let r = get(http, &format!("{base}/catalog.json"), false).await;
        let text = r.text().await.map_err(|e| e.to_string())?;
        if text.contains("\"waveform\"") {
            return Err("catalog.json contains waveform".into());
        }
        for id in &ids {
            if !text.contains(id.as_str()) {
                return Err(format!("catalog.json missing {id}"));
            }
        }
        Ok(())
    })
    .await?;
    println!("[c] catalog.json ok (3 ids, no waveform)");

    for t in &tracks {
        let text = get(http, &format!("{base}/meta/{}.json", t.id), false)
            .await
            .text()
            .await
            .map_err(|e| e.to_string())?;
        let meta: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let n = meta["waveform"].as_array().map_or(0, |a| a.len());
        if n != 800 {
            return Err(format!("meta/{}.json waveform has {n} values, want 800", t.id));
        }
    }
    println!("[c] meta/<id>.json waveform = 800 values");

    for t in &tracks {
        let r = get(http, &t.audio_url, true).await;
        if r.status().as_u16() != 206 {
            return Err(format!("audio Range -> {} (want 206)", r.status()));
        }
        if header(&r, "content-type") != "audio/mpeg" {
            return Err(format!("audio content-type {:?}", header(&r, "content-type")));
        }
        if !header(&r, "cache-control").contains("immutable") {
            return Err(format!("audio cache-control {:?}", header(&r, "cache-control")));
        }
        if r.headers().contains_key("content-encoding") {
            return Err("audio has Content-Encoding".into());
        }
    }
    println!("[c] audioUrl Range -> 206, audio/mpeg, immutable, no Content-Encoding");

    let cover_url = cover_big.cover_url.clone().ok_or("cover_big has no coverUrl")?;
    let r = get(http, &cover_url, false).await;
    if r.status().as_u16() != 200 || header(&r, "content-type") != "image/jpeg" {
        return Err(format!("coverUrl -> {} {}", r.status(), header(&r, "content-type")));
    }
    println!("[c] coverUrl -> 200 image/jpeg");

    // d. update title + swap cover
    let png = probe(&fixture("cover_small"), "cover_small")
        .map_err(|e| format!("{e:?}"))?
        .cover
        .ok_or("cover_small has no cover")?
        .bytes;
    let edit = TagEdit {
        title: format!("{} (e2e)", cover_big.title),
        artist: cover_big.artist.clone(),
        album: cover_big.album.clone(),
        album_artist: cover_big.album_artist.clone(),
        date: cover_big.date.clone(),
        genre: cover_big.genre.clone(),
        track: cover_big.track.clone(),
        disc: cover_big.disc.clone(),
    };
    let updated = lib
        .update(&cover_big.id, edit, Some(png))
        .await
        .map_err(|e| format!("update: {e:?}"))?;
    let new_url = updated.cover_url.clone().ok_or("updated track has no coverUrl")?;
    if updated.cover_key == cover_big.cover_key || new_url == cover_url {
        return Err("cover key did not change".into());
    }
    println!("[d] updated title to {:?}, new cover {new_url}", updated.title);
    eventually("new cover loads", || async {
        let c = status_of(http, &new_url).await;
        (c == 200).then_some(()).ok_or(format!("-> {c}"))
    })
    .await?;
    let still_used = lib
        .list()
        .await
        .map_err(|e| format!("{e:?}"))?
        .iter()
        .any(|t| t.id != cover_big.id && t.cover_key == cover_big.cover_key);
    if still_used {
        println!("[d] old cover shared with another track; skipping 404 check");
    } else {
        eventually("old cover gone", || async {
            let c = status_of(http, &cover_url).await;
            (c == 404).then_some(()).ok_or(format!("-> {c}"))
        })
        .await?;
        println!("[d] old cover object -> 404");
    }
    eventually("catalog shows new title", || async {
        let text = get(http, &format!("{base}/catalog.json"), false)
            .await
            .text()
            .await
            .map_err(|e| e.to_string())?;
        text.contains("(e2e)").then_some(()).ok_or("title not updated yet".to_string())
    })
    .await?;
    println!("[d] catalog.json shows the new title");

    // e. delete what we added
    let audio_urls: Vec<String> = tracks
        .iter()
        .filter(|t| added.contains(&t.id))
        .map(|t| t.audio_url.clone())
        .collect();
    lib.delete(added).await.map_err(|e| format!("delete: {e:?}"))?;
    println!("[e] deleted {} track(s)", added.len());
    let deleted = std::mem::take(added);
    for url in &audio_urls {
        eventually("audio gone", || async {
            let c = status_of(http, url).await;
            (c == 404).then_some(()).ok_or(format!("{url} -> {c}"))
        })
        .await?;
    }
    let after: Vec<String> = lib
        .list()
        .await
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .map(|t| t.id)
        .collect();
    if deleted.iter().any(|id| after.contains(id)) {
        return Err("deleted ids still in catalog".into());
    }
    for id in before {
        if !after.contains(id) {
            return Err(format!("pre-existing track {id} disappeared"));
        }
    }
    println!("[e] audioUrls -> 404, catalog no longer lists them; pre-existing tracks intact");
    Ok(())
}
