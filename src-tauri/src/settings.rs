use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub account_id: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub public_base_url: String,
}

impl Settings {
    /// Trims every field and strips trailing `/` from `public_base_url`.
    pub fn normalize(self) -> Self {
        Settings {
            account_id: self.account_id.trim().to_string(),
            access_key_id: self.access_key_id.trim().to_string(),
            secret_access_key: self.secret_access_key.trim().to_string(),
            bucket: self.bucket.trim().to_string(),
            public_base_url: self.public_base_url.trim().trim_end_matches('/').to_string(),
        }
    }

    pub fn is_complete(&self) -> bool {
        [
            &self.account_id,
            &self.access_key_id,
            &self.secret_access_key,
            &self.bucket,
            &self.public_base_url,
        ]
        .iter()
        .all(|f| !f.trim().is_empty())
    }

    /// Returns a copy whose secret is hidden except for its last 4 characters.
    pub fn masked(&self) -> Settings {
        let n = self.secret_access_key.chars().count();
        let secret = if n == 0 {
            String::new()
        } else if n <= 4 {
            "••••".to_string()
        } else {
            let tail: String = self.secret_access_key.chars().skip(n - 4).collect();
            format!("••••{tail}")
        };
        Settings { secret_access_key: secret, ..self.clone() }
    }
}

pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, s: &Settings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(s).map_err(io::Error::other)?;
    std::fs::write(path, json)
}
