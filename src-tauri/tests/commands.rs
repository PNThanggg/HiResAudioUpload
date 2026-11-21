use hires_upload_lib::catalog::{Library, MemoryStorage};
use hires_upload_lib::commands::{run_batch, UploadProgress, UploadStage};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn fx(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/tests/fixtures/{name}.mp3", env!("CARGO_MANIFEST_DIR")))
}

#[tokio::test]
async fn batch_emits_one_terminal_event_per_file() {
    let lib = Arc::new(Library::new(
        Arc::new(MemoryStorage::default()),
        "https://cdn.example.com".to_string(),
    ));
    let events: Arc<Mutex<Vec<UploadProgress>>> = Arc::default();
    let sink = events.clone();
    let paths = vec![
        fx("pink_128"),
        fx("pink_128"),
        fx("fake"),
        fx("short"),
        fx("cover_small"),
    ];
    run_batch(paths, lib.clone(), move |e| sink.lock().unwrap().push(e)).await;

    let ev = events.lock().unwrap().clone();
    let queued = ev.iter().filter(|e| e.stage == UploadStage::Queued).count();
    assert_eq!(queued, 5);
    let terminals: Vec<&UploadProgress> = ev
        .iter()
        .filter(|e| {
            matches!(
                e.stage,
                UploadStage::Done | UploadStage::Duplicate | UploadStage::Error
            )
        })
        .collect();
    assert_eq!(terminals.len(), 5);

    let pink = fx("pink_128").to_string_lossy().to_string();
    let of = |name: &str| -> Vec<&&UploadProgress> {
        let p = fx(name).to_string_lossy().to_string();
        terminals.iter().filter(|e| e.path == p).collect()
    };
    let mut pink_stages: Vec<_> = terminals
        .iter()
        .filter(|e| e.path == pink)
        .map(|e| e.stage)
        .collect();
    pink_stages.sort_by_key(|s| *s as u8);
    assert_eq!(pink_stages, vec![UploadStage::Done, UploadStage::Duplicate]);

    let fake = of("fake");
    assert_eq!(fake[0].stage, UploadStage::Error);
    assert_eq!(fake[0].error_code.as_deref(), Some("INVALID_FORMAT"));
    let short = of("short");
    assert_eq!(short[0].stage, UploadStage::Error);
    assert_eq!(short[0].error_code.as_deref(), Some("TOO_SHORT"));
    assert_eq!(of("cover_small")[0].stage, UploadStage::Done);

    assert_eq!(lib.list().await.unwrap().len(), 2);
}

#[tokio::test]
async fn oversized_file_is_rejected_before_reading() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("huge.mp3");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(hires_upload_lib::pipeline::probe::MAX_BYTES + 1)
        .unwrap();
    let lib = Arc::new(Library::new(
        Arc::new(MemoryStorage::default()),
        "https://cdn.example.com".to_string(),
    ));
    let events: Arc<Mutex<Vec<UploadProgress>>> = Arc::default();
    let sink = events.clone();
    run_batch(vec![path], lib, move |e| sink.lock().unwrap().push(e)).await;
    let ev = events.lock().unwrap().clone();
    let last = ev.last().unwrap();
    assert_eq!(last.stage, UploadStage::Error);
    assert_eq!(last.error_code.as_deref(), Some("TOO_LARGE"));
}
