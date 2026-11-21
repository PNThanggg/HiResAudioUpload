use hires_upload_lib::catalog::Storage;
use hires_upload_lib::r2::R2Storage;
use hires_upload_lib::settings::Settings;

fn env(k: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| panic!("missing env {k}"))
}

#[tokio::test]
#[ignore]
async fn r2_live() {
    let s = Settings {
        account_id: env("R2_ACCOUNT_ID"),
        access_key_id: env("R2_ACCESS_KEY_ID"),
        secret_access_key: env("R2_SECRET_ACCESS_KEY"),
        bucket: env("R2_BUCKET"),
        public_base_url: String::new(),
    };
    let r2 = R2Storage::new(&s).await;
    r2.test_connection().await.unwrap();
    let key = "test/probe.txt";
    r2.put(key, b"hello".to_vec(), "text/plain", "no-cache").await.unwrap();
    assert_eq!(r2.get(key).await.unwrap(), Some(b"hello".to_vec()));
    r2.delete(key).await.unwrap();
    assert_eq!(r2.get(key).await.unwrap(), None);
}
