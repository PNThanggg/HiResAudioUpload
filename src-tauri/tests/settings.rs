use hires_upload_lib::settings::{load, save, Settings};

fn sample() -> Settings {
    Settings {
        account_id: "acc".into(),
        access_key_id: "key".into(),
        secret_access_key: "supersecret1234".into(),
        bucket: "bkt".into(),
        public_base_url: "https://cdn.x.com".into(),
    }
}

#[test]
fn settings_normalize_trims_and_strips_slash() {
    let s = Settings {
        access_key_id: " key ".into(),
        public_base_url: "  https://cdn.x.com/ ".into(),
        ..Settings::default()
    }
    .normalize();
    assert_eq!(s.access_key_id, "key");
    assert_eq!(s.public_base_url, "https://cdn.x.com");
}

#[test]
fn masked_hides_secret() {
    assert_eq!(sample().masked().secret_access_key, "••••1234");
    assert_eq!(sample().masked().bucket, "bkt");
    let empty = Settings::default().masked();
    assert_eq!(empty.secret_access_key, "");
    let short = Settings { secret_access_key: "abc".into(), ..Settings::default() }.masked();
    assert_eq!(short.secret_access_key, "••••");
}

#[test]
fn is_complete_requires_all_fields() {
    assert!(sample().is_complete());
    assert!(!Settings { bucket: "  ".into(), ..sample() }.is_complete());
    assert!(!Settings::default().is_complete());
}

#[test]
fn load_missing_returns_default() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(load(&dir.path().join("nope.json")), Settings::default());
}

#[test]
fn load_invalid_json_returns_default() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("s.json");
    std::fs::write(&p, "{not json").unwrap();
    assert_eq!(load(&p), Settings::default());
}

#[test]
fn save_then_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("sub/dir/settings.json");
    save(&p, &sample()).unwrap();
    assert_eq!(load(&p), sample());
    let text = std::fs::read_to_string(&p).unwrap();
    assert!(text.contains("accountId"));
}

#[test]
fn validate_requires_http_public_base_url() {
    assert!(sample().validate().is_ok());
    assert!(Settings { public_base_url: "HTTP://cdn.x.com".into(), ..sample() }.validate().is_ok());
    assert!(Settings { public_base_url: "".into(), ..sample() }.validate().is_ok());
    for bad in ["cdn.x.com", "ftp://cdn.x.com", "javascript:alert(1)", "https:/x"] {
        let e = Settings { public_base_url: bad.into(), ..sample() }.validate().unwrap_err();
        assert!(e.contains("http://"), "{bad}: {e}");
    }
}
