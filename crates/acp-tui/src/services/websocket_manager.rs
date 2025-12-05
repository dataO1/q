//! WebSocket connection management with bidirectional communication and reconnection logic
//!
//! This module provides a robust WebSocket manager that:
//! - Establishes and maintains WebSocket connections with automatic reconnection
//! - Handles bidirectional communication (receiving StatusEvents, sending HITL decisions)
//! - Implements exponential backoff for reconnection attempts
//! - Provides comprehensive logging and instrumentation
//! - Manages subscription lifecycle

use std::sync::Arc;
use std::time::Duration;
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, instrument, warn};

use crate::{
    client::{AcpClient, StatusEvent },
    message::APIEvent,
    services::ApiService,
    utils::generate_client_id,
};

/// WebSocket connection state
#[derive(Debug, Clone, PartialEq)]
enum WsConnectionState {
    Disconnected,
    Connecting,
    Connected { subscription_id: String },
    Reconnecting { attempt: u32 },
}

/// Handle for sending messages through the WebSocket connection
type WsWriter = Arc<RwLock<Option<futures_util::stream::SplitSink<
    WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>>>>;

/// WebSocket connection manager with automatic reconnection and bidirectional communication
pub struct WebSocketManager {
    /// Base server URL (http/https)
    server_url: String,

    /// Channel sender for application events
    event_sender: mpsc::UnboundedSender<APIEvent>,

    /// Current reconnection attempt count
    reconnect_attempts: u32,

    /// Maximum reconnection attempts before giving up
    max_reconnect_attempts: u32,

    /// Current connection state
    state: Arc<RwLock<WsConnectionState>>,

    /// WebSocket writer for sending outgoing messages
    ws_writer: WsWriter,

    /// Client ID for this TUI instance
    client_id: String,
}

impl WebSocketManager {
    /// Create a new WebSocket manager
    ///
    /// # Arguments
    /// * `server_url` - Base HTTP/HTTPS URL of the ACP server
    /// * `event_sender` - Channel for sending application events
    ///
    /// # Returns
    /// Result containing the WebSocket manager instance
    #[instrument(skip(event_sender))]
    pub fn new(server_url: String, event_sender: mpsc::UnboundedSender<APIEvent>) -> Result<Self> {
        let client_id = generate_client_id()
            .context("Failed to generate client ID for WebSocket manager")?;

        info!(
            server_url = %server_url,
            client_id = %client_id,
            "Creating WebSocket manager"
        );

        Ok(Self {
            server_url,
            event_sender,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            state: Arc::new(RwLock::new(WsConnectionState::Disconnected)),
            ws_writer: Arc::new(RwLock::new(None)),
            client_id,
        })
    }

    /// Establish WebSocket connection with the server
    ///
    /// This method:
    /// 1. Creates a subscription with the server
    /// 2. Establishes the WebSocket connection
    /// 3. Spawns a task to handle incoming messages
    /// 4. Sets up the writer for outgoing messages
    #[instrument(skip(self))]
    pub async fn connect(&mut self) -> Result<()> {
        info!("Initiating WebSocket connection");

        // Update state to connecting
        {
            let mut state = self.state.write().await;
            *state = WsConnectionState::Connecting;
        }

        let _ = self.event_sender.send(APIEvent::StatusMessage(
            crate::message::StatusSeverity::Info,
            "Connecting to WebSocket...".to_string(),
        ));

        // Create subscription
        let subscription_id = self.create_subscription()
            .await
            .context("Failed to create WebSocket subscription")?;

        debug!(subscription_id = %subscription_id, "Subscription created");

        // Build WebSocket URL
        let ws_url = self.build_websocket_url(&subscription_id)?;

        info!(ws_url = %ws_url, "Connecting to WebSocket endpoint");

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .context("Failed to establish WebSocket connection")?;

        info!(subscription_id = %subscription_id, "WebSocket connected successfully");

        // Update state to connected
        {
            let mut state = self.state.write().await;
            *state = WsConnectionState::Connected {
                subscription_id: subscription_id.clone(),
            };
        }

        // Notify application of successful connection
        let _ = self.event_sender.send(APIEvent::WebSocketConnected(subscription_id.clone()));

        // Split stream for concurrent read/write
        let (write, read) = ws_stream.split();

        // Store writer for sending messages
        {
            let mut writer = self.ws_writer.write().await;
            *writer = Some(write);
        }

        // Spawn message reader task
        self.spawn_reader_task(read, subscription_id).await;

        // Reset reconnection attempts on successful connection
        self.reconnect_attempts = 0;

        Ok(())
    }

    /// Create a subscription with the server
    #[instrument(skip(self))]
    async fn create_subscription(&self) -> Result<String> {
        let client = AcpClient::new(&self.server_url)
            .context("Failed to create ACP client")?;

        let api_service = ApiService::new(Arc::new(client));

        let subscription_id = api_service
            .create_subscription(self.client_id.clone())
            .await
            .context("Failed to create subscription via API")?;

        debug!(
            subscription_id = %subscription_id,
            client_id = %self.client_id,
            "Subscription created successfully"
        );

        Ok(subscription_id)
    }

    /// Build WebSocket URL from base server URL and subscription ID
    #[instrument(skip(self))]
    fn build_websocket_url(&self, subscription_id: &str) -> Result<String> {
        let client = AcpClient::new(&self.server_url)
            .context("Failed to create ACP client for WebSocket URL")?;

        Ok(client.get_websocket_url(subscription_id))
    }

    /// Spawn a task to handle incoming WebSocket messages
    #[instrument(skip(self, read))]
    async fn spawn_reader_task(
        &self,
        mut read: futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>,
        subscription_id: String,
    ) {
        let event_sender = self.event_sender.clone();
        let state = Arc::clone(&self.state);

        tokio::spawn(async move {
            info!(subscription_id = %subscription_id, "📥 WebSocket reader task started");
            let mut message_count = 0u64;

            // 🔥 ADD THIS: Log that we're entering the loop
            info!("🔄 Entering WebSocket message receive loop");

            while let Some(result) = read.next().await {
                // 🔥 ADD THIS: Log EVERY iteration
                debug!("🔄 Received WebSocket message result");

                match result {
                    Ok(Message::Text(text)) => {
                        message_count += 1;

                        // 🔥 CRITICAL: Log raw message ALWAYS
                        info!(
                            message_count,
                            message_len = text.len(),
                            message_preview = %text.chars().take(200).collect::<String>(),
                            "📥 Raw WebSocket TEXT message received"
                        );

                        // Parse StatusEvent
                        match serde_json::from_str::<StatusEvent>(&text) {
                            Ok(event) => {
                                info!(
                                    event_type = ?event.event,
                                    id = %event.id,
                                    "✅ Parsed StatusEvent successfully"
                                );
                                let _ = event_sender.send(APIEvent::StatusEventReceived(event));
                            }
                            Err(e) => {
                                warn!(
                                    error = %e,
                                    message_preview = %text.chars().take(100).collect::<String>(),
                                    "❌ Failed to parse StatusEvent from WebSocket message"
                                );
                            }
                        }
                    }
                    Ok(Message::Close(close_frame)) => {
                        info!(
                            close_frame = ?close_frame,
                            messages_received = message_count,
                            "🔌 WebSocket connection closed by server"
                        );
                        let mut state_guard = state.write().await;
                        *state_guard = WsConnectionState::Disconnected;
                        let _ = event_sender.send(APIEvent::WebSocketDisconnected);
                        break;
                    }
                    Ok(Message::Ping(data)) => {
                        info!(data_length = data.len(), "🏓 Received ping");
                    }
                    Ok(Message::Pong(_)) => {
                        debug!("🏓 Received pong");
                    }
                    Ok(msg) => {
                        info!(message_type = ?msg, "📨 Received other message type");
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            error_debug = ?e,
                            messages_received = message_count,
                            "❌ WebSocket error occurred"
                        );
                        let mut state_guard = state.write().await;
                        *state_guard = WsConnectionState::Disconnected;
                        let _ = event_sender.send(APIEvent::WebSocketDisconnected);
                        break;
                    }
                }
            }

            // 🔥 ADD THIS: Log when loop exits
            info!(
                total_messages = message_count,
                subscription_id = %subscription_id,
                "🔚 WebSocket reader loop EXITED (stream ended)"
            );
        });

        // 🔥 ADD THIS: Log that spawn completed
        info!("✅ Reader task spawned successfully");
    }

    /// Disconnect the WebSocket connection gracefully
    #[instrument(skip(self))]
    pub async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting WebSocket");

        // Send close message if writer is available
        if let Some(mut writer) = self.ws_writer.write().await.take() {
            let _ = writer.send(Message::Close(None)).await;
            let _ = writer.close().await;
            debug!("Close message sent");
        }

        // Update state
        {
            let mut state = self.state.write().await;
            *state = WsConnectionState::Disconnected;
        }

        Ok(())
    }

    /// Attempt to reconnect with exponential backoff
    #[instrument(skip(self))]
    pub async fn reconnect(&mut self) -> Result<()> {
        if self.reconnect_attempts >= self.max_reconnect_attempts {
            error!(
                attempts = self.reconnect_attempts,
                max_attempts = self.max_reconnect_attempts,
                "Maximum reconnection attempts exceeded"
            );
            return Err(anyhow::anyhow!("Max reconnection attempts exceeded"));
        }

        self.reconnect_attempts += 1;

        // Update state
        {
            let mut state = self.state.write().await;
            *state = WsConnectionState::Reconnecting {
                attempt: self.reconnect_attempts,
            };
        }

        // Calculate exponential backoff delay (2^attempt seconds, max 32s)
        let delay = Duration::from_secs(2_u64.pow(self.reconnect_attempts.min(5)));

        warn!(
            attempt = self.reconnect_attempts,
            max_attempts = self.max_reconnect_attempts,
            delay_secs = delay.as_secs(),
            "Attempting WebSocket reconnection"
        );

        let _ = self.event_sender.send(APIEvent::StatusMessage(
            crate::message::StatusSeverity::Warning,
            format!(
                "Reconnecting... (attempt {}/{})",
                self.reconnect_attempts, self.max_reconnect_attempts
            ),
        ));

        sleep(delay).await;

        self.connect().await
    }

    /// Reset reconnection attempt counter
    ///
    /// Call this when manually initiating a reconnection to reset the exponential backoff
    pub fn reset_reconnect_attempts(&mut self) {
        debug!(
            previous_attempts = self.reconnect_attempts,
            "Resetting reconnection attempts"
        );
        self.reconnect_attempts = 0;
    }

    /// Submit a HITL decision through the WebSocket connection
    ///
    /// # Arguments
    /// * `decision` - The HITL decision request to send
    ///
    /// # Returns
    /// Result indicating success or failure
    #[instrument(skip(self), fields(request_id = %decision.id))]
    pub async fn submit_hitl_decision(&self, decision: StatusEvent) -> Result<()> {
        info!(
            id = %decision.id,
            decision_type = ?decision.event,
            "Submitting HITL decision via WebSocket"
        );

        // Check connection state
        let state = self.state.read().await;
        match &*state {
            WsConnectionState::Connected { subscription_id } => {
                debug!(subscription_id = %subscription_id, "Connection active, proceeding with send");
            }
            other_state => {
                warn!(state = ?other_state, "WebSocket not connected, cannot send HITL decision");
                return Err(anyhow::anyhow!(
                    "WebSocket not connected (state: {:?})",
                    other_state
                ));
            }
        }
        drop(state);

        // Serialize decision to JSON
        let message_json = serde_json::to_string(&decision)
            .context("Failed to serialize HITL decision to JSON")?;

        debug!(
            message_length = message_json.len(),
            "Serialized HITL decision"
        );

        // Send through WebSocket
        let mut writer_guard = self.ws_writer.write().await;

        if let Some(writer) = writer_guard.as_mut() {
            writer
                .send(Message::Text(message_json.into()))
                .await
                .context("Failed to send HITL decision through WebSocket")?;

            info!(id = %decision.id, "HITL decision sent successfully");
            Ok(())
        } else {
            warn!("WebSocket writer not available");
            Err(anyhow::anyhow!("WebSocket writer not available"))
        }
    }

    /// Submit a StatusEvent through the WebSocket connection
    ///
    /// Generic method for sending any StatusEvent (useful for testing or custom events)
    #[instrument(skip(self), fields(event_type = ?event.event))]
    pub async fn send_status_event(&self, event: StatusEvent) -> Result<()> {
        debug!(
            event_type = ?event.event,
            id = %event.id,
            "Sending StatusEvent via WebSocket"
        );

        let message_json = serde_json::to_string(&event)
            .context("Failed to serialize StatusEvent to JSON")?;

        let mut writer_guard = self.ws_writer.write().await;

        if let Some(writer) = writer_guard.as_mut() {
            writer
                .send(Message::Text(message_json.into()))
                .await
                .context("Failed to send StatusEvent through WebSocket")?;

            debug!("StatusEvent sent successfully");
            Ok(())
        } else {
            warn!("WebSocket writer not available");
            Err(anyhow::anyhow!("WebSocket writer not available"))
        }
    }

    /// Check if WebSocket is currently connected
    pub async fn is_connected(&self) -> bool {
        matches!(*self.state.read().await, WsConnectionState::Connected { .. })
    }

    /// Get current subscription ID if connected
    pub async fn get_subscription_id(&self) -> Option<String> {
        if let WsConnectionState::Connected { subscription_id } = &*self.state.read().await {
            Some(subscription_id.clone())
        } else {
            None
        }
    }
}

impl Drop for WebSocketManager {
    fn drop(&mut self) {
        debug!("WebSocketManager dropped");
    }
}
