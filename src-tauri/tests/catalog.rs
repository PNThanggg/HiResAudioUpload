mod common;

use common::fixture;
use hires_upload_lib::catalog::*;
use hires_upload_lib::pipeline::{process, sha256_hex, ProcessedTrack};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const IMMUTABLE: &str = "public, max-age=31536000, immutable";

fn track(name: &str) -> ProcessedTrack {
    process(fixture(name), name, &AtomicBool::new(false)).unwrap()
}

fn setup() -> (Arc<MemoryStorage>, Library) {
    let st = Arc::new(MemoryStorage::default());
    let lib = Library::new(st.clone(), "https://cdn.example.com".to_string());
    (st, lib)
}

fn edit(title: &str) -> TagEdit {
    TagEdit {
        title: title.to_string(),
        artist: Some("  Artist ".to_string()),
        album: Some("".to_string()),
        album_artist: None,
        date: None,
        genre: None,
        track: None,
        disc: None,
    }
}

fn added(o: AddOutcome) -> TrackSummary {
    match o {
        AddOutcome::Added(t) => t,
        AddOutcome::Duplicate(_) => panic!("expected Added"),
    }
}

fn covers(st: &MemoryStorage) -> Vec<String> {
    st.keys()
        .into_iter()
        .filter(|k| k.starts_with("cover/"))
        .collect()
}

#[tokio::test]
async fn missing_catalog_is_empty() {
    let (_st, lib) = setup();
    assert!(lib.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn add_writes_objects_with_headers_then_catalog() {
    let (st, lib) = setup();
    let p = track("cover_small");
    let t = added(lib.add(p.clone()).await.unwrap());
    let cover = p.cover.as_ref().unwrap();
    let ckey = format!("cover/{}.{}", sha256_hex(&cover.bytes), cover.ext);
    let akey = format!("audio/{}.mp3", p.id);
    let mkey = format!("meta/{}.json", p.id);
    let keys = st.keys();
    for k in [&akey, &ckey, &mkey] {
        assert!(keys.contains(k), "missing {k}");
    }
    assert!(keys.contains(&"catalog.json".to_string()));
    assert_eq!(t.audio_key, akey);
    assert_eq!(t.audio_url, format!("https://cdn.example.com/{akey}"));
    assert_eq!(t.cover_key.as_deref(), Some(ckey.as_str()));
    assert_eq!(t.size_bytes, p.served.len() as u64);

    assert_eq!(
        st.object_headers(&akey).unwrap(),
        ("audio/mpeg".to_string(), IMMUTABLE.to_string())
    );
    assert_eq!(
        st.object_headers(&ckey).unwrap(),
        (cover.mime.to_string(), IMMUTABLE.to_string())
    );
    assert_eq!(
        st.object_headers(&mkey).unwrap(),
        ("application/json".to_string(), "no-cache".to_string())
    );
    assert_eq!(
        st.object_headers("catalog.json").unwrap(),
        ("application/json".to_string(), "no-cache".to_string())
    );

    let cat: serde_json::Value =
        serde_json::from_slice(&st.get("catalog.json").await.unwrap().unwrap()).unwrap();
    let first = &cat["tracks"][0];
    assert!(first.get("waveform").is_none());
    assert_eq!(cat["version"], 1);
    assert_eq!(first["sourceSha256"], p.source_sha256);
    assert!(first.get("truePeakDbtp").is_some());
    assert!(first.get("qualityHint").is_some());
    let meta: serde_json::Value =
        serde_json::from_slice(&st.get(&mkey).await.unwrap().unwrap()).unwrap();
    assert_eq!(meta["waveform"].as_array().unwrap().len(), 800);
    assert_eq!(lib.detail(&p.id).await.unwrap().waveform.len(), 800);
}

#[tokio::test]
async fn same_file_twice_is_duplicate() {
    let (_st, lib) = setup();
    let t = added(lib.add(track("pink_128")).await.unwrap());
    match lib.add(track("pink_128")).await.unwrap() {
        AddOutcome::Duplicate(d) => assert_eq!(d.id, t.id),
        AddOutcome::Added(_) => panic!("expected Duplicate"),
    }
    assert_eq!(lib.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn find_by_source_after_add() {
    let (_st, lib) = setup();
    let p = track("pink_128");
    assert!(lib.find_by_source(&p.source_sha256).await.unwrap().is_none());
    let t = added(lib.add(p.clone()).await.unwrap());
    assert_eq!(lib.find_by_source(&p.source_sha256).await.unwrap(), Some(t));
    assert!(lib.find_by_source("nope").await.unwrap().is_none());
}

#[tokio::test]
async fn failed_put_leaves_catalog_unchanged() {
    let (st, lib) = setup();
    *st.fail_puts_matching.lock().unwrap() = Some("catalog.json".to_string());
    assert!(lib.add(track("cover_small")).await.is_err());
    assert!(
        !st.keys()
            .iter()
            .any(|k| k.starts_with("audio/") || k.starts_with("meta/") || k.starts_with("cover/")),
        "leftover keys: {:?}",
        st.keys()
    );

    *st.fail_puts_matching.lock().unwrap() = None;
    lib.add(track("pink_128")).await.unwrap();
    lib.add(track("pink_320")).await.unwrap();
    *st.fail_puts_matching.lock().unwrap() = Some("audio/".to_string());
    assert!(lib.add(track("pink_v2")).await.is_err());
    assert_eq!(lib.list().await.unwrap().len(), 2);
}

#[tokio::test]
async fn corrupt_catalog_blocks_writes() {
    let (st, lib) = setup();
    st.put(
        "catalog.json",
        b"{not json".to_vec(),
        "application/json",
        "no-cache",
    )
    .await
    .unwrap();
    assert!(matches!(
        lib.list().await,
        Err(LibraryError::CatalogCorrupt(_))
    ));
    assert!(matches!(
        lib.add(track("pink_128")).await,
        Err(LibraryError::CatalogCorrupt(_))
    ));
    assert!(matches!(
        lib.delete(&["x".to_string()]).await,
        Err(LibraryError::CatalogCorrupt(_))
    ));
    assert_eq!(st.get("catalog.json").await.unwrap().unwrap(), b"{not json");
}

#[tokio::test]
async fn update_changes_tags_and_swaps_cover() {
    let (st, lib) = setup();
    let t = added(lib.add(track("cover_small")).await.unwrap());
    let old_cover = t.cover_key.clone().unwrap();
    let new_img = track("cover_big").cover.unwrap().bytes;

    let u = lib
        .update(&t.id, edit("  New title "), Some(new_img))
        .await
        .unwrap();
    assert_eq!(u.title, "New title");
    assert_eq!(u.artist.as_deref(), Some("Artist"));
    assert_eq!(u.album, None);
    let new_cover = u.cover_key.clone().unwrap();
    assert_ne!(new_cover, old_cover);
    assert!(new_cover.ends_with(".jpg"));
    assert_eq!(
        u.cover_url.as_deref(),
        Some(format!("https://cdn.example.com/{new_cover}").as_str())
    );
    assert!(st.get(&old_cover).await.unwrap().is_none());
    assert!(st.get(&new_cover).await.unwrap().is_some());
    let rec = lib.detail(&t.id).await.unwrap();
    assert_eq!(rec.summary, u);
    assert_eq!(rec.waveform.len(), 800);
    assert_eq!(lib.list().await.unwrap(), vec![u]);

    assert!(matches!(
        lib.update(&t.id, edit("   "), None).await,
        Err(LibraryError::Validation(_))
    ));
    assert!(matches!(
        lib.update(&t.id, edit("x"), Some(b"not an image".to_vec()))
            .await,
        Err(LibraryError::Validation(_))
    ));
    assert!(matches!(
        lib.update("missing", edit("x"), None).await,
        Err(LibraryError::NotFound(_))
    ));
}

#[tokio::test]
async fn shared_cover_survives_single_delete() {
    let (st, lib) = setup();
    let a = added(lib.add(track("pink_128")).await.unwrap());
    let b = added(lib.add(track("pink_320")).await.unwrap());
    let img = track("cover_small").cover.unwrap().bytes;
    lib.update(&a.id, edit("a"), Some(img.clone()))
        .await
        .unwrap();
    lib.update(&b.id, edit("b"), Some(img)).await.unwrap();
    assert_eq!(covers(&st).len(), 1);

    lib.delete(std::slice::from_ref(&a.id)).await.unwrap();
    assert_eq!(covers(&st).len(), 1);
    assert!(!st.keys().contains(&format!("audio/{}.mp3", a.id)));
    assert!(!st.keys().contains(&format!("meta/{}.json", a.id)));
    assert!(matches!(
        lib.detail(&a.id).await,
        Err(LibraryError::NotFound(_))
    ));

    lib.delete(&[b.id.clone(), "unknown".to_string()])
        .await
        .unwrap();
    assert!(covers(&st).is_empty());
    assert!(!st.keys().contains(&format!("audio/{}.mp3", b.id)));
    assert!(lib.list().await.unwrap().is_empty());
}
