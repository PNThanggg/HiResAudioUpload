use anyhow::Context;
use async_trait::async_trait;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{
    BehaviorVersion, Credentials, Region, RequestChecksumCalculation, ResponseChecksumValidation,
};
use aws_sdk_s3::primitives::ByteStream;

use crate::catalog::Storage;
use crate::settings::Settings;

pub struct R2Storage {
    client: Client,
    bucket: String,
}

impl R2Storage {
    pub async fn new(s: &Settings) -> Self {
        let config = aws_sdk_s3::config::Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("auto"))
            .endpoint_url(format!("https://{}.r2.cloudflarestorage.com", s.account_id))
            .credentials_provider(Credentials::new(
                s.access_key_id.clone(),
                s.secret_access_key.clone(),
                None,
                None,
                "r2-settings",
            ))
            .force_path_style(true)
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired)
            .build();
        R2Storage { client: Client::from_conf(config), bucket: s.bucket.clone() }
    }

    pub async fn test_connection(&self) -> anyhow::Result<()> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("head_bucket failed: {}", aws_sdk_s3::error::DisplayErrorContext(e)))?;
        Ok(())
    }
}

#[async_trait]
impl Storage for R2Storage {
    async fn put(
        &self,
        key: &str,
        body: Vec<u8>,
        content_type: &str,
        cache_control: &str,
    ) -> anyhow::Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(body))
            .content_type(content_type)
            .cache_control(cache_control)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("put {key} failed: {}", aws_sdk_s3::error::DisplayErrorContext(e)))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        match self.client.get_object().bucket(&self.bucket).key(key).send().await {
            Ok(out) => {
                let bytes = out
                    .body
                    .collect()
                    .await
                    .with_context(|| format!("read body of {key}"))?;
                Ok(Some(bytes.into_bytes().to_vec()))
            }
            Err(e) if e.as_service_error().map(|s| s.is_no_such_key()).unwrap_or(false) => Ok(None),
            Err(e) => Err(anyhow::anyhow!(
                "get {key} failed: {}",
                aws_sdk_s3::error::DisplayErrorContext(e)
            )),
        }
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("delete {key} failed: {}", aws_sdk_s3::error::DisplayErrorContext(e)))?;
        Ok(())
    }
}
