//! # NATS Publisher Worker
//!
//! Background task responsible for reliably publishing events
//! to NATS JetStream.

use async_nats::jetstream;
use async_nats::jetstream::stream::Config;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::infrastructure::messaging::dispatcher::PublishPayload;
use serde_json::Value;
use std::collections::HashMap;

/// Background worker for publishing events to NATS JetStream.
pub struct NatsPublisherWorker {
    client: async_nats::Client,
    jetstream: jetstream::Context,
    receiver: mpsc::Receiver<PublishPayload>,
    stream_name: String,
    subject_prefix: String,
}

/// Extracts NATS headers from event metadata.
///
/// This is a pure function that converts event metadata into NATS headers,
/// including correlation_id and schema_version when present.
pub fn extract_nats_headers(metadata: &Value, _subject: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();

    // Extract correlation_id from event_id
    if let Some(e_id) = metadata.get("event_id").and_then(|v| v.as_str()) {
        headers.insert("correlation_id".to_string(), e_id.to_string());
    }

    // Extract schema_version
    if let Some(schema_version) = metadata.get("schema_version")
        && let Some(obj) = schema_version.as_object()
        && let (Some(major), Some(minor), Some(patch)) = (
            obj.get("major").and_then(|v| v.as_u64()),
            obj.get("minor").and_then(|v| v.as_u64()),
            obj.get("patch").and_then(|v| v.as_u64()),
        )
    {
        let version_str = format!("{}.{}.{}", major, minor, patch);
        headers.insert("schema_version".to_string(), version_str);
    }

    headers
}

impl NatsPublisherWorker {
    /// Connects to NATS and initializes the JetStream context and stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection to NATS fails, or if JetStream stream creation fails.
    pub async fn connect(
        nats_url: &str,
        stream_name: &str,
        subject_prefix: &str,
        receiver: mpsc::Receiver<PublishPayload>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = async_nats::connect(nats_url).await?;
        let jetstream = jetstream::new(client.clone());

        // Ensure the stream exists
        let subjects = vec![format!("{}.>", subject_prefix)];

        match jetstream.get_stream(stream_name).await {
            Ok(mut stream) => {
                info!("Found existing JetStream stream: {}", stream_name);
                // Validate that the existing stream is configured with the required subject.
                let required_subject = format!("{}.>", subject_prefix);
                let info = stream.info().await?;
                let has_required_subject =
                    info.config.subjects.iter().any(|s| s == &required_subject);

                if !has_required_subject {
                    error!(
                        "JetStream stream '{}' exists but is missing required subject '{}'",
                        stream_name, required_subject
                    );
                    return Err(format!(
                        "JetStream stream '{}' is misconfigured: missing subject '{}'",
                        stream_name, required_subject
                    )
                    .into());
                }
            }
            Err(err) => {
                if err.to_string().to_lowercase().contains("stream not found") {
                    info!(
                        "Creating JetStream stream: {} for subjects {:?}",
                        stream_name, subjects
                    );
                    jetstream
                        .create_stream(Config {
                            name: stream_name.to_string(),
                            subjects,
                            ..Default::default()
                        })
                        .await?;
                } else {
                    return Err(Box::new(err));
                }
            }
        }

        Ok(Self {
            client,
            jetstream,
            receiver,
            stream_name: stream_name.to_string(),
            subject_prefix: subject_prefix.to_string(),
        })
    }

    /// Health check for the NATS connection.
    ///
    /// # Errors
    ///
    /// Returns an error if the NATS client is disconnected.
    pub fn health_check(&self) -> Result<(), String> {
        use async_nats::connection::State;
        if self.client.connection_state() == State::Connected {
            Ok(())
        } else {
            Err("NATS client is disconnected".to_string())
        }
    }

    /// Starts the worker loop to consume events and publish to NATS.
    pub async fn run(mut self) {
        info!(
            "Starting NatsPublisherWorker for stream {} (prefix: {})",
            self.stream_name, self.subject_prefix
        );

        while let Some((subject, payload)) = self.receiver.recv().await {
            self.publish_with_retry(&subject, &payload).await;
        }

        info!("NatsPublisherWorker shutting down (channel closed)");
    }

    /// Publishes an event to JetStream with exponential backoff retry.
    async fn publish_with_retry(&self, subject: &str, payload: &str) {
        let max_retries = 5;
        let mut retry_count = 0;
        let mut delay = Duration::from_millis(100);
        let payload_bytes = bytes::Bytes::from(payload.to_string());

        let mut headers = async_nats::HeaderMap::new();
        if let Some(event_type) = subject.split('.').next_back() {
            headers.insert("event_type", event_type);
        }

        #[allow(clippy::collapsible_if)]
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) {
            if let Some(metadata) = json.get("metadata") {
                // Extract timestamp (legacy logic)
                if let Some(ts) = metadata.get("timestamp") {
                    if let Some(ts_str) = ts.as_str() {
                        headers.insert("timestamp", ts_str);
                    } else if let Some(ts_u64) = ts.as_u64() {
                        headers.insert("timestamp", ts_u64.to_string().as_str());
                    }
                }

                // Extract other headers using helper function
                let extracted_headers = extract_nats_headers(metadata, subject);
                for (key, value) in extracted_headers {
                    headers.insert(key.as_str(), value.as_str());
                }
            }
        }

        loop {
            match self
                .jetstream
                .publish_with_headers(subject.to_string(), headers.clone(), payload_bytes.clone())
                .await
            {
                Ok(ack) => match ack.await {
                    Ok(publish_ack) => {
                        debug!(
                            "Successfully published event to {} (seq: {})",
                            subject, publish_ack.sequence
                        );
                        return;
                    }
                    Err(e) => warn!("JetStream ack error for {}: {}", subject, e),
                },
                Err(e) => warn!("Failed to publish to {}: {}", subject, e),
            }

            retry_count += 1;
            if retry_count > max_retries {
                error!("Exceeded max retries ({}). Writing to DLQ.", max_retries);
                if let Err(e) = Self::write_to_dlq(subject, payload, None).await {
                    error!("DLQ write failed for {}: {}", subject, e);
                }
                return;
            }

            warn!(
                "Retrying publish to {} in {}ms (attempt {}/{})",
                subject,
                delay.as_millis(),
                retry_count,
                max_retries
            );
            tokio::time::sleep(delay).await;
            delay = delay.saturating_mul(2);
        }
    }

    async fn write_to_dlq(
        subject: &str,
        payload: &str,
        path_override: Option<&str>,
    ) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;
        let env_path =
            std::env::var("NATS_DLQ_PATH").unwrap_or_else(|_| "nats_dlq.txt".to_string());
        let dlq_path = path_override.unwrap_or(&env_path);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dlq_path)
            .await?;
        let entry = format!("[{}] {}\n", subject, payload);
        file.write_all(entry.as_bytes()).await?;
        file.sync_all().await?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_to_dlq() {
        let subject = "test.dlq.subject";
        let payload = r#"{"test":"dlq_payload"}"#;

        let temp_dir = std::env::temp_dir();
        let dlq_path = temp_dir
            .join(format!("nats_dlq_{}.txt", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .to_string();

        let result = NatsPublisherWorker::write_to_dlq(subject, payload, Some(&dlq_path)).await;
        assert!(result.is_ok());

        let contents = tokio::fs::read_to_string(&dlq_path).await.unwrap();
        assert!(contents.contains(&format!("[{}] {}\n", subject, payload)));

        let _ = tokio::fs::remove_file(&dlq_path).await;
    }

    #[tokio::test]
    async fn test_extract_nats_headers_with_schema_version() {
        let metadata = serde_json::json!({
            "event_id": "550e8400-e29b-41d4-a716-446655440000",
            "rfq_id": "660f9501-f39c-52e5-b827-557766551111",
            "timestamp": "2023-01-01T00:00:00Z",
            "schema_version": {
                "major": 1,
                "minor": 2,
                "patch": 3
            }
        });

        let headers = extract_nats_headers(&metadata, "otc.test.RfqCreated");

        assert_eq!(
            headers.get("correlation_id"),
            Some(&"550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        assert_eq!(headers.get("schema_version"), Some(&"1.2.3".to_string()));
        assert_eq!(headers.len(), 2);
    }

    #[tokio::test]
    async fn test_extract_nats_headers_without_schema_version() {
        let metadata = serde_json::json!({
            "event_id": "550e8400-e29b-41d4-a716-446655440000",
            "rfq_id": "660f9501-f39c-52e5-b827-557766551111",
            "timestamp": "2023-01-01T00:00:00Z"
        });

        let headers = extract_nats_headers(&metadata, "otc.test.RfqCreated");

        assert_eq!(
            headers.get("correlation_id"),
            Some(&"550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        assert_eq!(headers.get("schema_version"), None);
        assert_eq!(headers.len(), 1);
    }

    #[tokio::test]
    async fn test_extract_nats_headers_with_malformed_schema_version() {
        let metadata = serde_json::json!({
            "event_id": "550e8400-e29b-41d4-a716-446655440000",
            "schema_version": "not_an_object"
        });

        let headers = extract_nats_headers(&metadata, "otc.test.RfqCreated");

        assert_eq!(
            headers.get("correlation_id"),
            Some(&"550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        assert_eq!(headers.get("schema_version"), None);
        assert_eq!(headers.len(), 1);
    }
}
