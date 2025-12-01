//! WebSocket connection management with reconnection logic

use std::time::Duration;
use anyhow::Result;
use tokio::{sync::mpsc, time::sleep};
use tracing::{error, info, warn};

use crate::{
    message::AppMsg,
    components::realm::status_line::ConnectionState,
};

/// WebSocket connection manager with automatic reconnection
pub struct WebSocketManager {
    server_url: String,
    sender: mpsc::UnboundedSender<AppMsg>,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
}

impl WebSocketManager {
    /// Create new WebSocket manager
    pub fn new(server_url: String, sender: mpsc::UnboundedSender<AppMsg>) -> Self {
        Self {
            server_url,
            sender,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
        }
    }
    
    /// Start WebSocket connection with proper URL
    pub async fn connect(&mut self) -> Result<()> {
        info!("Starting WebSocket connection to {}", self.server_url);
        
        // Fix the WebSocket URL - use proper WebSocket endpoint
        let _ws_url = self.server_url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
            + "/ws";
        
        let _ = self.sender.send(AppMsg::StatusMessage(
            crate::message::StatusSeverity::Info,
            "Connecting to WebSocket...".to_string()
        ));
        
        // Implement proper WebSocket connection using the subscription_id and generated client
        use tokio_tungstenite::{connect_async, tungstenite::Message};
        use futures_util::StreamExt;
        
        // First, create a subscription to get the subscription_id
        let api_service = crate::services::ApiService::new(
            std::sync::Arc::new(crate::client::AcpClient::new(&self.server_url)?)
        );
        
        // Generate a stable client ID based on hardware
        let client_id = crate::utils::generate_client_id()?;
        
        match api_service.create_subscription(client_id).await {
            Ok(subscription_id) => {
                info!("WebSocket subscription created: {}", subscription_id);
                
                // Create WebSocket URL using the subscription_id
                let acp_client = crate::client::AcpClient::new(&self.server_url)?;
                let ws_url = acp_client.get_websocket_url(&subscription_id);
                
                info!("Connecting to WebSocket: {}", ws_url);
                
                // Connect to WebSocket  
                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        info!("WebSocket connected successfully");
                        let _ = self.sender.send(AppMsg::WebSocketConnected);
                        
                        // Split the stream for concurrent reading and writing
                        let (_write, mut read) = ws_stream.split();
                        
                        // Spawn task to handle incoming messages
                        let message_sender = self.sender.clone();
                        tokio::spawn(async move {
                            while let Some(msg) = read.next().await {
                                match msg {
                                    Ok(Message::Text(text)) => {
                                        // Parse the JSON message as StatusEvent
                                        match serde_json::from_str::<crate::client::types::StatusEvent>(&text) {
                                            Ok(event) => {
                                                let _ = message_sender.send(AppMsg::StatusEventReceived(event));
                                            }
                                            Err(e) => {
                                                warn!("Failed to parse WebSocket message: {}", e);
                                            }
                                        }
                                    }
                                    Ok(Message::Close(_)) => {
                                        info!("WebSocket connection closed");
                                        let _ = message_sender.send(AppMsg::WebSocketDisconnected);
                                        break;
                                    }
                                    Err(e) => {
                                        error!("WebSocket error: {}", e);
                                        let _ = message_sender.send(AppMsg::WebSocketDisconnected);
                                        break;
                                    }
                                    _ => {} // Ignore other message types
                                }
                            }
                        });
                        
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to connect to WebSocket: {}", e);
                        let _ = self.sender.send(AppMsg::ConnectionFailed(e.to_string()));
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                error!("Failed to create WebSocket subscription: {}", e);
                let _ = self.sender.send(AppMsg::ConnectionFailed(e.to_string()));
                Err(e)
            }
        }
    }
    
    /// Disconnect WebSocket
    pub async fn disconnect(&mut self) -> Result<()> {
        // WebSocket connection is now handled through tokio tasks
        // The connection will be dropped when the task ends
        info!("Disconnecting WebSocket");
        Ok(())
    }
    
    /// Attempt to reconnect with exponential backoff
    pub async fn reconnect(&mut self) -> Result<()> {
        if self.reconnect_attempts >= self.max_reconnect_attempts {
            error!("Max reconnection attempts reached");
            return Err(anyhow::anyhow!("Max reconnection attempts exceeded"));
        }
        
        self.reconnect_attempts += 1;
        let delay = Duration::from_secs(2_u64.pow(self.reconnect_attempts.min(5)));
        
        warn!(
            "Attempting to reconnect (attempt {}/{}) in {:?}",
            self.reconnect_attempts, self.max_reconnect_attempts, delay
        );
        
        let _ = self.sender.send(AppMsg::StatusMessage(
            crate::message::StatusSeverity::Warning,
            format!("Reconnecting... (attempt {})", self.reconnect_attempts)
        ));
        
        sleep(delay).await;
        self.connect().await
    }
    
    /// Check if connected
    pub fn is_connected(&self) -> bool {
        // For now, assume connected if we haven't hit max reconnect attempts
        self.reconnect_attempts < self.max_reconnect_attempts
    }
    
    /// Get connection state for UI
    pub fn get_connection_state(&self) -> ConnectionState {
        if self.is_connected() {
            ConnectionState::Connected
        } else if self.reconnect_attempts > 0 {
            ConnectionState::Reconnecting {
                attempt: self.reconnect_attempts as usize,
            }
        } else {
            ConnectionState::Disconnected
        }
    }
    
    /// Reset reconnection attempts (call when manually reconnecting)
    pub fn reset_reconnect_attempts(&mut self) {
        self.reconnect_attempts = 0;
    }
}