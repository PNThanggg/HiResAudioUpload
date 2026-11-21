#![allow(dead_code)]

pub fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}.mp3", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OracleEntry {
    pub lufs: f64,
    pub lra: f64,
    pub true_peak_dbtp: f64,
}

/// `tests/fixtures/oracle.json`, keyed by fixture stem (".mp3" suffix removed).
pub fn oracle() -> std::collections::HashMap<String, OracleEntry> {
    let path = format!("{}/tests/fixtures/oracle.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let raw: std::collections::HashMap<String, OracleEntry> = serde_json::from_str(&text).unwrap();
    raw.into_iter()
        .map(|(k, v)| (k.trim_end_matches(".mp3").to_string(), v))
        .collect()
}
