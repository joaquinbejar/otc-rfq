-- Add notification preferences to counterparties table
-- Migration: V003
-- Description: Add notification channels and configuration for multi-channel trade confirmations

-- Add notification preference columns
ALTER TABLE counterparties 
ADD COLUMN notification_channels TEXT[] DEFAULT '{}',
ADD COLUMN notification_email VARCHAR(255),
ADD COLUMN notification_webhook_url VARCHAR(512),
ADD COLUMN notification_grpc_endpoint VARCHAR(255);

-- Add comments for documentation
COMMENT ON COLUMN counterparties.notification_channels IS 'Array of enabled notification channels (WEBSOCKET, EMAIL, API_CALLBACK, GRPC)';
COMMENT ON COLUMN counterparties.notification_email IS 'Email address for email notifications';
COMMENT ON COLUMN counterparties.notification_webhook_url IS 'Webhook URL for API callback notifications';
COMMENT ON COLUMN counterparties.notification_grpc_endpoint IS 'gRPC endpoint for gRPC streaming notifications';

-- Create index for faster lookups on notification channels
CREATE INDEX idx_counterparties_notification_channels 
ON counterparties USING GIN (notification_channels);

-- Add constraint to ensure valid channel names
ALTER TABLE counterparties
ADD CONSTRAINT chk_notification_channels 
CHECK (
    notification_channels <@ ARRAY['WEBSOCKET', 'EMAIL', 'API_CALLBACK', 'GRPC']::TEXT[]
);

-- Add constraint to ensure email is provided if EMAIL channel is enabled
ALTER TABLE counterparties
ADD CONSTRAINT chk_email_required_for_email_channel
CHECK (
    NOT ('EMAIL' = ANY(notification_channels)) OR notification_email IS NOT NULL
);

-- Add constraint to ensure webhook URL is provided if API_CALLBACK channel is enabled
ALTER TABLE counterparties
ADD CONSTRAINT chk_webhook_required_for_api_callback
CHECK (
    NOT ('API_CALLBACK' = ANY(notification_channels)) OR notification_webhook_url IS NOT NULL
);

-- Add constraint to ensure gRPC endpoint is provided if GRPC channel is enabled
ALTER TABLE counterparties
ADD CONSTRAINT chk_grpc_endpoint_required_for_grpc
CHECK (
    NOT ('GRPC' = ANY(notification_channels)) OR notification_grpc_endpoint IS NOT NULL
);
