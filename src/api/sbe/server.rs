//! # SBE TCP Server
//!
//! TCP server for SBE protocol communication.
//!
//! This module provides a length-prefixed framing layer on top of the SBE
//! message codecs, handling connection lifecycle, message dispatch, and
//! graceful shutdown.
//!
//! ## Channel Architecture
//!
//! Each connection uses two separate outbound channels to prevent streaming
//! backpressure from starving request/response traffic:
//!
//! - **Control-plane** (`ctrl_tx`): bounded channel for request/response frames.
//!   Reader sends via `.send().await` — strict delivery, never drops.
//! - **Data-plane** (`data_tx`): bounded channel for streaming updates.
//!   Forwarder sends via `try_send()` — lossy under pressure, drops with warning.
//!
//! The writer task drains both channels using `select! biased`, always
//! prioritizing control-plane frames over data-plane frames.

// Buffer bounds are validated before indexing
#![allow(clippy::indexing_slicing)]

use crate::api::sbe::error::{SbeApiError, SbeApiResult};
use crate::api::sbe::handlers;
use crate::api::sbe::subscriptions::SessionSubscriptions;
use crate::api::sbe::types::*;
use crate::application::use_cases::create_rfq::RfqRepository;
use crate::infrastructure::sbe::{SbeDecode, SbeEncode};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, Semaphore, broadcast, mpsc, watch};
use tracing::{error, info, warn};

/// Capacity of the control-plane channel (request/response frames).
///
/// Kept small — each request produces exactly one response, so this only
/// needs to absorb a brief burst while the writer is flushing.
const CTRL_CHANNEL_CAPACITY: usize = 32;

/// Capacity of the data-plane channel (streaming update frames).
///
/// Larger than control-plane to absorb quote/status bursts. When full,
/// the forwarder drops updates via `try_send` rather than blocking.
const DATA_CHANNEL_CAPACITY: usize = 256;

/// SBE server configuration.
#[derive(Debug, Clone)]
pub struct SbeConfig {
    /// Maximum concurrent connections.
    pub max_connections: usize,
    /// Read timeout in seconds.
    pub read_timeout_secs: u64,
    /// Maximum message size in bytes.
    pub max_message_size: usize,
    /// Maximum subscriptions per connection.
    pub max_subscriptions: usize,
}

impl Default for SbeConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            read_timeout_secs: 30,
            max_message_size: 1024 * 1024, // 1 MB
            max_subscriptions: 100,
        }
    }
}

/// Shared application state for SBE handlers.
#[derive(Clone)]
pub struct AppState {
    /// RFQ repository for persistence.
    pub rfq_repository: Arc<dyn RfqRepository>,
    /// Event broadcaster for quote updates.
    pub quote_updates: broadcast::Sender<QuoteUpdate>,
    /// Event broadcaster for RFQ status updates.
    pub status_updates: broadcast::Sender<RfqStatusUpdate>,
}

impl AppState {
    /// Publishes a quote update event.
    pub fn publish_quote_update(&self, update: QuoteUpdate) {
        let _ = self.quote_updates.send(update);
    }

    /// Publishes an RFQ status update event.
    pub fn publish_status_update(&self, update: RfqStatusUpdate) {
        let _ = self.status_updates.send(update);
    }
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

/// Handles a single client connection with subscription support.
///
/// Uses two outbound channels to isolate control-plane (request/response)
/// from data-plane (streaming updates) traffic. This prevents streaming
/// backpressure from blocking request responses.
///
/// Spawns three concurrent tasks:
/// - **Reader**: reads frames, handles subscribe/unsubscribe, dispatches
///   requests, sends responses on `ctrl_tx`
/// - **Forwarder**: filters broadcast events and pushes to `data_tx` via
///   `try_send` (lossy — drops under pressure with warning)
/// - **Writer**: drains both channels with `select! biased`, always
///   prioritizing control-plane frames
///
/// # Errors
///
/// Returns error on IO failure or protocol violation.
async fn handle_connection(
    stream: TcpStream,
    state: Arc<AppState>,
    config: &SbeConfig,
) -> anyhow::Result<()> {
    let (reader, writer) = stream.into_split();

    // Control-plane channel: request/response frames (strict delivery)
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<Vec<u8>>(CTRL_CHANNEL_CAPACITY);

    // Data-plane channel: streaming update frames (lossy under pressure)
    let (data_tx, mut data_rx) = mpsc::channel::<Vec<u8>>(DATA_CHANNEL_CAPACITY);

    // Per-connection subscription state
    let subscriptions = Arc::new(RwLock::new(SessionSubscriptions::new(
        config.max_subscriptions,
    )));

    // Subscribe to broadcast channels
    let mut quote_rx = state.quote_updates.subscribe();
    let mut status_rx = state.status_updates.subscribe();

    // Clone for forwarder task
    let subs_for_forwarder = Arc::clone(&subscriptions);

    // Spawn event forwarder task (data-plane, lossy)
    let forwarder = tokio::spawn(async move {
        loop {
            tokio::select! {
                res = quote_rx.recv() => {
                    match res {
                        Ok(update) => {
                            let subs = subs_for_forwarder.read().await;
                            if subs.is_subscribed_quotes(&update.rfq_id)
                                && let Ok(encoded) = update.encode_to_vec()
                                && let Err(mpsc::error::TrySendError::Full(_)) = data_tx.try_send(encoded)
                            {
                                warn!("data channel full, dropped quote update");
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!(skipped, "quote_rx lagged, skipped updates");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                res = status_rx.recv() => {
                    match res {
                        Ok(update) => {
                            let subs = subs_for_forwarder.read().await;
                            if subs.is_subscribed_status(&update.rfq_id)
                                && let Ok(encoded) = update.encode_to_vec()
                                && let Err(mpsc::error::TrySendError::Full(_)) = data_tx.try_send(encoded)
                            {
                                warn!("data channel full, dropped status update");
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!(skipped, "status_rx lagged, skipped updates");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    });

    // Spawn writer task (prioritizes control-plane over data-plane)
    let writer_task = tokio::spawn(async move {
        let mut writer = writer;
        loop {
            tokio::select! {
                biased;
                frame = ctrl_rx.recv() => match frame {
                    Some(f) => {
                        if write_frame(&mut writer, &f).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                },
                frame = data_rx.recv() => match frame {
                    Some(f) => {
                        if write_frame(&mut writer, &f).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                },
            }
        }
    });

    // Reader task (runs in current task, control-plane)
    let reader_result = handle_reader(reader, state, config, subscriptions, ctrl_tx).await;

    // Cleanup
    forwarder.abort();
    writer_task.abort();

    reader_result
}

/// Handles reading frames and processing requests/subscriptions.
///
/// Sends all response frames on the control-plane channel (`ctrl_tx`).
async fn handle_reader(
    mut reader: tokio::net::tcp::OwnedReadHalf,
    state: Arc<AppState>,
    config: &SbeConfig,
    subscriptions: Arc<RwLock<SessionSubscriptions>>,
    ctrl_tx: mpsc::Sender<Vec<u8>>,
) -> anyhow::Result<()> {
    let read_timeout = std::time::Duration::from_secs(config.read_timeout_secs);

    loop {
        // Read frame with timeout
        let frame = match tokio::time::timeout(
            read_timeout,
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
                // Timeout: check if we have active subscriptions
                let subs = subscriptions.read().await;
                if subs.count() > 0 {
                    // Keep connection alive for subscribed clients
                    continue;
                } else {
                    warn!("Read timeout with no active subscriptions");
                    break;
                }
            }
        };

        // Extract template ID to route subscription messages
        if frame.len() < MESSAGE_HEADER_SIZE {
            continue;
        }
        let template_id = u16::from_le_bytes([frame[2], frame[3]]);

        // Handle subscription messages locally
        let response = match template_id {
            SUBSCRIBE_QUOTES_REQUEST_TEMPLATE_ID => match SubscribeQuotesRequest::decode(&frame) {
                Ok(req) => {
                    let mut subs = subscriptions.write().await;
                    match subs.subscribe_quotes(req.rfq_id) {
                        Ok(()) => subscription_ack(req.rfq_id, "subscribed to quotes")?,
                        Err(e) => error_to_response(e).encode_to_vec()?,
                    }
                }
                Err(e) => error_to_response(SbeApiError::SbeEncoding(e)).encode_to_vec()?,
            },
            SUBSCRIBE_RFQ_STATUS_REQUEST_TEMPLATE_ID => {
                match SubscribeRfqStatusRequest::decode(&frame) {
                    Ok(req) => {
                        let mut subs = subscriptions.write().await;
                        match subs.subscribe_status(req.rfq_id) {
                            Ok(()) => subscription_ack(req.rfq_id, "subscribed to status")?,
                            Err(e) => error_to_response(e).encode_to_vec()?,
                        }
                    }
                    Err(e) => error_to_response(SbeApiError::SbeEncoding(e)).encode_to_vec()?,
                }
            }
            UNSUBSCRIBE_REQUEST_TEMPLATE_ID => match UnsubscribeRequest::decode(&frame) {
                Ok(req) => {
                    let mut subs = subscriptions.write().await;
                    subs.unsubscribe(req.rfq_id);
                    subscription_ack(req.rfq_id, "unsubscribed")?
                }
                Err(e) => error_to_response(SbeApiError::SbeEncoding(e)).encode_to_vec()?,
            },
            _ => {
                // Dispatch regular request-response messages
                match dispatch_message(&frame, &state).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!(error = %e, "Dispatch error");
                        error_to_response(e).encode_to_vec()?
                    }
                }
            }
        };

        // Send response on control-plane channel
        if ctrl_tx.send(response).await.is_err() {
            break;
        }
    }

    Ok(())
}

/// Creates a subscription acknowledgment using ErrorResponse with code 0.
///
/// # Errors
///
/// Returns error if encoding fails.
#[inline]
fn subscription_ack(rfq_id: uuid::Uuid, message: &str) -> SbeApiResult<Vec<u8>> {
    let ack = ErrorResponse {
        request_id: uuid::Uuid::nil(),
        rfq_id,
        code: 0,
        message: message.to_string(),
        metadata: String::new(),
    };
    ack.encode_to_vec().map_err(SbeApiError::SbeEncoding)
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
        CANCEL_RFQ_REQUEST_TEMPLATE_ID => {
            let request = CancelRfqRequest::decode(frame)?;
            let response = handlers::handle_cancel_rfq(request, state).await?;
            response.encode_to_vec().map_err(SbeApiError::SbeEncoding)
        }
        EXECUTE_TRADE_REQUEST_TEMPLATE_ID => {
            let request = ExecuteTradeRequest::decode(frame)?;
            let response = handlers::handle_execute_trade(request, state).await?;
            response.encode_to_vec().map_err(SbeApiError::SbeEncoding)
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
        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        let state = Arc::new(AppState {
            rfq_repository: Arc::new(MockRfqRepository::new()),
            quote_updates,
            status_updates,
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
    async fn dispatch_cancel_rfq_not_found() {
        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        let state = Arc::new(AppState {
            rfq_repository: Arc::new(MockRfqRepository::new()),
            quote_updates,
            status_updates,
        });

        let request = CancelRfqRequest {
            request_id: uuid::Uuid::new_v4(),
            rfq_id: uuid::Uuid::new_v4(),
            reason: "test".to_string(),
        };

        let mut frame = vec![0u8; request.encoded_size()];
        request.encode(&mut frame).unwrap();

        let result = dispatch_message(&frame, &state).await;
        assert!(matches!(result, Err(SbeApiError::InvalidValue { .. })));
    }

    #[tokio::test]
    async fn dispatch_execute_trade_not_found() {
        let (quote_updates, _) = tokio::sync::broadcast::channel(16);
        let (status_updates, _) = tokio::sync::broadcast::channel(16);
        let state = Arc::new(AppState {
            rfq_repository: Arc::new(MockRfqRepository::new()),
            quote_updates,
            status_updates,
        });

        let request = ExecuteTradeRequest {
            rfq_id: uuid::Uuid::new_v4(),
            quote_id: uuid::Uuid::new_v4(),
        };

        let mut frame = vec![0u8; request.encoded_size()];
        request.encode(&mut frame).unwrap();

        let result = dispatch_message(&frame, &state).await;
        assert!(matches!(result, Err(SbeApiError::InvalidValue { .. })));
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
