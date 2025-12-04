use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade, Message}, Path, State},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use tracing::{debug, error, info, warn, instrument};
use crate::server::AppState;

/// WebSocket handler for streaming status updates
#[instrument(skip(ws, state), fields(subscription_id = %subscription_id))]
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(subscription_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    info!(
        subscription_id = %subscription_id,
        "WebSocket connection upgrade requested"
    );

    ws.on_upgrade(move |socket| handle_socket(socket, subscription_id, state))
}

/// Handle individual WebSocket connection - BIDIRECTIONAL streaming
#[instrument(skip(socket, state), fields(subscription_id = %subscription_id))]
async fn handle_socket(socket: WebSocket, subscription_id: String, state: AppState) {
    info!(
        subscription_id = %subscription_id,
        "WebSocket connection established, starting BIDIRECTIONAL status stream"
    );

    // Split socket into sender and receiver for BIDIRECTIONAL communication
    let (mut sender, mut receiver) = socket.split();

    // Connect to the subscription and get stream with buffered events replayed
    let mut status_receiver = match state.execution_manager.connect_subscription(&subscription_id).await {
        Some(receiver) => receiver,
        None => {
            error!("No subscription found for {}", subscription_id);
            return;
        }
    };

    // Clone subscription_id for use in tasks
    let sub_id_sender = subscription_id.clone();
    let sub_id_receiver = subscription_id.clone();

    // OUTBOUND task: stream status events to client (server → client)
    let sender_task = tokio::spawn(async move {
        while let Ok(event) = status_receiver.recv().await {
            match serde_json::to_string(&event) {
                Ok(json) => {
                    info!("📤 Sending event to WebSocket client: {:?}", event.event);
                    if let Err(e) = sender.send(Message::Text(json.into())).await {
                        warn!("Failed to send WebSocket message: {}", e);
                        break;
                    }
                    debug!("Sent status event for subscription {}", sub_id_sender);
                }
                Err(e) => {
                    error!("Failed to serialize status event: {}", e);
                    break;
                }
            }
        }
        debug!("WebSocket sender task ended for subscription {}", sub_id_sender);
    });

    // 🔥 INBOUND task: receive events from client (client → server) 🔥
    let state_clone = state.clone();
    let receiver_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    debug!("Received inbound message: {} bytes", text.len());

                    // Parse the StatusEvent from client (e.g., HitlDecision)
                    match serde_json::from_str::<ai_agent_common::StatusEvent>(&text) {
                        Ok(event) => {
                            info!("Parsed inbound event: {:?}", event.event);

                            // Route to subscription (which routes to waiting agents via event_waiters)
                            let mut subscriptions = state_clone.execution_manager.subscriptions.write().await;
                            if let Some(subscription) = subscriptions.get_mut(&sub_id_receiver) {
                                if let Err(e) = subscription.receive_inbound(event).await {
                                    warn!("Failed to route inbound event: {}", e);
                                } else {
                                    info!("✅ Successfully routed inbound event to waiting agent");
                                }
                            } else {
                                warn!("Subscription not found for inbound routing: {}", sub_id_receiver);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse inbound event: {} - data: {}", e, text);
                        }
                    }
                }
                Message::Close(_) => {
                    info!("Client closed WebSocket connection");
                    break;
                }
                Message::Ping(data) => {
                    debug!("Received ping, sending pong");
                    // Axum handles pong automatically
                }
                _ => {
                    debug!("Received other message type: {:?}", msg);
                }
            }
        }
        debug!("WebSocket receiver task ended for subscription {}", sub_id_receiver);
    });

    // Wait for either task to complete (connection closed or error)
    tokio::select! {
        result = sender_task => {
            if let Err(e) = result {
                warn!("WebSocket sender task error for subscription {}: {}", subscription_id, e);
            }
        }
        result = receiver_task => {
            if let Err(e) = result {
                warn!("WebSocket receiver task error for subscription {}: {}", subscription_id, e);
            }
        }
    }

    // Mark subscription as disconnected but keep it alive for potential reconnection
    state.execution_manager.disconnect_subscription(&subscription_id).await;
    info!(
        "WebSocket connection closed for subscription (subscription kept alive for reconnection): {}",
        subscription_id
    );
}
