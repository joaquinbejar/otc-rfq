//! # SBE TCP Server
//!
//! TCP server for SBE protocol communication.
//!
//! This module provides a length-prefixed framing layer on top of the SBE
//! message codecs, handling connection lifecycle, message dispatch, and
//! graceful shutdown.

// Buffer bounds are validated before indexing
#![allow(clippy::indexing_slicing)]

use crate::api::sbe::error::{SbeApiError, SbeApiResult};
use crate::api::sbe::handlers;
use crate::api::sbe::types::*;
use crate::application::use_cases::create_rfq::RfqRepository;
use crate::infrastructure::sbe::{SbeDecode, SbeEncode};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};
use tracing::{error, info, warn};

/// SBE server configuration.
#[derive(Debug, Clone)]
pub struct SbeConfig {
    /// Maximum concurrent connections.
    pub max_connections: usize,
    /// Read timeout in seconds.
    pub read_timeout_secs: u64,
    /// Maximum message size in bytes.
    pub max_message_size: usize,
}

impl Default for SbeConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            read_timeout_secs: 30,
            max_message_size: 1024 * 1024, // 1 MB
        }
    }
}

/// Shared application state for SBE handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// RFQ repository for persistence.
    pub rfq_repository: Arc<dyn RfqRepository>,
}

/// SBE TCP server.
pub struct SbeServer {
    listener: TcpListener,
    state: Arc<AppState>,
    shutdown_rx: watch::Receiver<bool>,
    config: SbeConfig,
    connection_semaphore: Arc<Semaphore>,
}

impl SbeServer {
    /// Creates a new SBE server.
    #[must_use]
    pub fn new(
        listener: TcpListener,
        state: Arc<AppState>,
        shutdown_rx: watch::Receiver<bool>,
        config: SbeConfig,
    ) -> Self {
        let connection_semaphore = Arc::new(Semaphore::new(config.max_connections));
        Self {
            listener,
            state,
            shutdown_rx,
            config,
            connection_semaphore,
        }
    }

    /// Runs the server until shutdown signal.
    ///
    /// # Errors
    ///
    /// Returns error if listener fails or shutdown handling fails.
    pub async fn run(mut self) -> anyhow::Result<()> {
        info!(
            addr = %self.listener.local_addr()?,
            max_connections = self.config.max_connections,
            max_message_size = self.config.max_message_size,
            "SBE server started"
        );

        loop {
            tokio::select! {
                accept_result = self.listener.accept() => {
                    match accept_result {
                        Ok((stream, peer_addr)) => {
                            // Acquire connection permit
                            let permit = match Arc::clone(&self.connection_semaphore).try_acquire_owned() {
                                Ok(p) => p,
                                Err(_) => {
                                    warn!(
                                        peer = %peer_addr,
                                        max_connections = self.config.max_connections,
                                        "Connection limit reached, rejecting connection"
                                    );
                                    continue;
                                }
                            };

                            let state = Arc::clone(&self.state);
                            let config = self.config.clone();

                            tokio::spawn(async move {
                                let _permit = permit; // Hold permit for connection lifetime
                                info!(peer = %peer_addr, "Client connected");

                                if let Err(e) = handle_connection(stream, state, &config).await {
                                    warn!(peer = %peer_addr, error = %e, "Connection error");
                                }

                                info!(peer = %peer_addr, "Client disconnected");
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to accept connection");
                        }
                    }
                }
                shutdown_result = self.shutdown_rx.changed() => {
                    match shutdown_result {
                        Ok(()) => {
                            if *self.shutdown_rx.borrow_and_update() {
                                info!("Shutdown signal received, stopping SBE server");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "Shutdown watch channel closed; stopping SBE server");
                            break;
                        }
                    }
                }
            }
        }

        info!("SBE server stopped");
        Ok(())
    }
}

/// Handles a single client connection.
///
/// # Errors
///
/// Returns error on IO failure or protocol violation.
async fn handle_connection(
    mut stream: TcpStream,
    state: Arc<AppState>,
    config: &SbeConfig,
) -> anyhow::Result<()> {
    let (reader, writer) = stream.split();
    let mut reader = reader;
    let mut writer = writer;

    loop {
        // Read frame with timeout
        let frame = match tokio::time::timeout(
            std::time::Duration::from_secs(config.read_timeout_secs),
            read_frame(&mut reader, config.max_message_size),
        )
        .await
        {
            Ok(Ok(f)) => f,
            Ok(Err(SbeApiError::ConnectionClosed)) => break,
            Ok(Err(e)) => {
                warn!(error = %e, "Frame read error");
                return Err(e.into());
            }
            Err(_) => {
                warn!("Read timeout");
                break;
            }
        };

        // Dispatch message
        let response = match dispatch_message(&frame, &state).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "Dispatch error");
                // Try to send error response
                let error_response = error_to_response(e);
                match error_response.encode_to_vec() {
                    Ok(encoded) => encoded,
                    Err(encode_err) => {
                        error!(error = %encode_err, "Failed to encode error response");
                        return Err(encode_err.into());
                    }
                }
            }
        };

        // Write response
        if let Err(e) = write_frame(&mut writer, &response).await {
            warn!(error = %e, "Frame write error");
            return Err(e.into());
        }
    }

    Ok(())
}

/// Reads a length-prefixed frame from the stream.
///
/// # Errors
///
/// Returns `SbeApiError::FrameTooLarge` if length > max_size.
/// Returns `SbeApiError::ConnectionClosed` on EOF.
/// Returns `SbeApiError::Io` on read failure.
#[inline]
async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_size: usize,
) -> SbeApiResult<Vec<u8>> {
    // Read 4-byte big-endian length prefix
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(SbeApiError::ConnectionClosed);
        }
        Err(e) => return Err(SbeApiError::Io(e)),
    }

    let length = u32::from_be_bytes(len_buf) as usize;

    // Validate length
    if length > max_size {
        return Err(SbeApiError::FrameTooLarge {
            size: length,
            max: max_size,
        });
    }

    // Read message payload
    let mut buffer = vec![0u8; length];
    match reader.read_exact(&mut buffer).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(SbeApiError::ConnectionClosed);
        }
        Err(e) => return Err(SbeApiError::Io(e)),
    }

    Ok(buffer)
}

/// Writes a length-prefixed frame to the stream.
///
/// # Errors
///
/// Returns `SbeApiError::FrameTooLarge` if data.len() > u32::MAX.
/// Returns `SbeApiError::Io` on write failure.
#[inline]
async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, data: &[u8]) -> SbeApiResult<()> {
    // Validate length fits in u32
    let length = u32::try_from(data.len()).map_err(|_| SbeApiError::FrameTooLarge {
        size: data.len(),
        max: u32::MAX as usize,
    })?;

    // Write length prefix (big-endian)
    writer.write_all(&length.to_be_bytes()).await?;

    // Write message payload
    writer.write_all(data).await?;

    Ok(())
}

/// Dispatches an SBE message to the appropriate handler.
///
/// # Errors
///
/// Returns `SbeApiError` for decode failures or unsupported templates.
async fn dispatch_message(frame: &[u8], state: &AppState) -> SbeApiResult<Vec<u8>> {
    // Decode header to get template ID
    if frame.len() < MESSAGE_HEADER_SIZE {
        return Err(SbeApiError::SbeEncoding(
            crate::infrastructure::sbe::SbeError::BufferTooSmall {
                needed: MESSAGE_HEADER_SIZE,
                available: frame.len(),
            },
        ));
    }

    let template_id = u16::from_le_bytes([frame[2], frame[3]]);

    match template_id {
        CREATE_RFQ_REQUEST_TEMPLATE_ID => {
            let request = CreateRfqRequest::decode(frame)?;
            let response = handlers::handle_create_rfq(request, state).await?;
            response.encode_to_vec().map_err(SbeApiError::SbeEncoding)
        }
        GET_RFQ_REQUEST_TEMPLATE_ID => {
            let request = GetRfqRequest::decode(frame)?;
            let response = handlers::handle_get_rfq(request, state).await?;
            response.encode_to_vec().map_err(SbeApiError::SbeEncoding)
        }
        // Deferred to #123
        CANCEL_RFQ_REQUEST_TEMPLATE_ID | EXECUTE_TRADE_REQUEST_TEMPLATE_ID => {
            Err(SbeApiError::NotImplemented(template_id))
        }
        // Deferred to #124
        SUBSCRIBE_QUOTES_REQUEST_TEMPLATE_ID
        | SUBSCRIBE_RFQ_STATUS_REQUEST_TEMPLATE_ID
        | UNSUBSCRIBE_REQUEST_TEMPLATE_ID => Err(SbeApiError::NotImplemented(template_id)),
        _ => Err(SbeApiError::SbeEncoding(
            crate::infrastructure::sbe::SbeError::UnknownTemplateId(template_id),
        )),
    }
}

/// Converts an error to an ErrorResponse.
#[inline(never)]
#[cold]
fn error_to_response(err: SbeApiError) -> ErrorResponse {
    use uuid::Uuid;

    let (code, message) = match &err {
        SbeApiError::SbeEncoding(e) => match e {
            crate::infrastructure::sbe::SbeError::UnknownTemplateId(id) => {
                (400, format!("unknown template id: {}", id))
            }
            crate::infrastructure::sbe::SbeError::InvalidEnumValue(v) => {
                (400, format!("invalid enum value: {}", v))
            }
            _ => (400, "invalid message format".to_string()),
        },
        SbeApiError::InvalidValue { field, message } => {
            (400, format!("invalid {}: {}", field, message))
        }
        SbeApiError::FrameTooLarge { size, max } => {
            (413, format!("frame too large: {} > {}", size, max))
        }
        SbeApiError::NotImplemented(_) => (501, "not implemented".to_string()),
        _ => (500, "internal error".to_string()),
    };

    ErrorResponse {
        request_id: Uuid::nil(),
        rfq_id: Uuid::nil(),
        code,
        message,
        metadata: String::new(),
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::useless_vec,
    clippy::clone_on_ref_ptr
)]
mod tests {
    use super::*;
    use crate::domain::entities::rfq::Rfq;
    use crate::domain::value_objects::RfqId;
    use std::collections::HashMap;
    use std::io::Cursor;
    use tokio::sync::RwLock;

    #[derive(Debug)]
    struct MockRfqRepository {
        rfqs: RwLock<HashMap<RfqId, Rfq>>,
    }

    impl MockRfqRepository {
        fn new() -> Self {
            Self {
                rfqs: RwLock::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl RfqRepository for MockRfqRepository {
        async fn save(&self, rfq: &Rfq) -> Result<(), String> {
            self.rfqs.write().await.insert(rfq.id(), rfq.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: RfqId) -> Result<Option<Rfq>, String> {
            Ok(self.rfqs.read().await.get(&id).cloned())
        }
    }

    #[tokio::test]
    async fn read_frame_success() {
        let data = vec![1, 2, 3, 4, 5];
        let length = data.len() as u32;
        let mut frame = length.to_be_bytes().to_vec();
        frame.extend_from_slice(&data);

        let mut cursor = Cursor::new(frame);
        let result = read_frame(&mut cursor, 1024).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data);
    }

    #[tokio::test]
    async fn read_frame_too_large() {
        let length = 2048u32;
        let frame = length.to_be_bytes().to_vec();

        let mut cursor = Cursor::new(frame);
        let result = read_frame(&mut cursor, 1024).await;

        assert!(matches!(result, Err(SbeApiError::FrameTooLarge { .. })));
    }

    #[tokio::test]
    async fn read_frame_eof() {
        let frame = vec![0, 0]; // Incomplete length prefix
        let mut cursor = Cursor::new(frame);
        let result = read_frame(&mut cursor, 1024).await;

        assert!(matches!(result, Err(SbeApiError::ConnectionClosed)));
    }

    #[tokio::test]
    async fn write_frame_success() {
        let data = vec![1, 2, 3, 4, 5];
        let mut buffer = Vec::new();

        let result = write_frame(&mut buffer, &data).await;
        assert!(result.is_ok());

        let expected_length = (data.len() as u32).to_be_bytes();
        assert_eq!(&buffer[0..4], &expected_length);
        assert_eq!(&buffer[4..], &data);
    }

    #[tokio::test]
    async fn write_frame_roundtrip() {
        let original = vec![10, 20, 30, 40, 50];
        let mut buffer = Vec::new();

        write_frame(&mut buffer, &original).await.unwrap();

        let mut cursor = Cursor::new(buffer);
        let decoded = read_frame(&mut cursor, 1024).await.unwrap();

        assert_eq!(decoded, original);
    }

    #[tokio::test]
    async fn dispatch_unknown_template() {
        let state = Arc::new(AppState {
            rfq_repository: Arc::new(MockRfqRepository::new()),
        });

        // Create a frame with invalid template ID
        let mut frame = vec![0u8; MESSAGE_HEADER_SIZE];
        frame[2] = 99; // Invalid template ID
        frame[3] = 0;

        let result = dispatch_message(&frame, &state).await;
        assert!(matches!(
            result,
            Err(SbeApiError::SbeEncoding(
                crate::infrastructure::sbe::SbeError::UnknownTemplateId(99)
            ))
        ));
    }

    #[tokio::test]
    async fn dispatch_not_implemented_template() {
        let state = Arc::new(AppState {
            rfq_repository: Arc::new(MockRfqRepository::new()),
        });

        // Create a frame with CancelRfq template (deferred to #123)
        let mut frame = vec![0u8; MESSAGE_HEADER_SIZE];
        let template_id = CANCEL_RFQ_REQUEST_TEMPLATE_ID;
        frame[2] = (template_id & 0xFF) as u8;
        frame[3] = (template_id >> 8) as u8;

        let result = dispatch_message(&frame, &state).await;
        assert!(matches!(result, Err(SbeApiError::NotImplemented(_))));
    }

    #[test]
    fn error_to_response_unknown_template() {
        let err =
            SbeApiError::SbeEncoding(crate::infrastructure::sbe::SbeError::UnknownTemplateId(999));
        let response = error_to_response(err);

        assert_eq!(response.code, 400);
        assert!(response.message.contains("unknown template id"));
    }

    #[test]
    fn error_to_response_not_implemented() {
        let err = SbeApiError::NotImplemented(24);
        let response = error_to_response(err);

        assert_eq!(response.code, 501);
        assert_eq!(response.message, "not implemented");
    }

    #[test]
    fn error_to_response_frame_too_large() {
        let err = SbeApiError::FrameTooLarge {
            size: 2048,
            max: 1024,
        };
        let response = error_to_response(err);

        assert_eq!(response.code, 413);
        assert!(response.message.contains("frame too large"));
    }
}
