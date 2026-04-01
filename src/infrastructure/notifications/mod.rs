//! # Notification Infrastructure
//!
//! Channel adapters for multi-channel trade confirmation delivery.

pub mod api_callback_confirmation;
pub mod email_confirmation;
pub mod grpc_confirmation;
pub mod websocket_confirmation;

pub use api_callback_confirmation::ApiCallbackConfirmationAdapter;
pub use email_confirmation::EmailConfirmationAdapter;
pub use grpc_confirmation::GrpcConfirmationAdapter;
pub use websocket_confirmation::WebSocketConfirmationAdapter;
