//! Execution Manager - handles conversation streams and orchestrator lifecycle
//!
//! The ExecutionManager provides:
//! - Bidirectional status stream management using Tokio channels
//! - Async execution of queries via stateless Orchestrator
//! - Session lifecycle and cleanup
//! - Generic StatusEvent routing (server ↔ client)

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use ai_agent_common::llm::EmbeddingClient;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use uuid::Uuid;
use anyhow::{Context, Result};
use tracing::{debug, info, warn, error, instrument};
use chrono::{DateTime, Utc};

use crate::agents::AgentPool;
use crate::coordination::CoordinationManager;
use crate::filelocks::FileLockManager;
use crate::hitl::AuditLogger;
use crate::orchestrator::Orchestrator;
use crate::sharedcontext::SharedContext;
use ai_agent_common::{
    ConversationId, ProjectScope, SystemConfig, StatusEvent, EventSource, EventType,
};
use ai_agent_history::HistoryManager;
use ai_agent_rag::SmartMultiSourceRag;

/// Bidirectional event channel for communication between server components and WebSocket clients
/// This is the CORE communication primitive - all events flow through this channel
#[derive(Clone)]
pub struct BidirectionalEventChannel {
    /// Unique identifier for this channel
    id: String,

    // === OUTBOUND (Server → Client) ===
    /// Broadcast sender for outbound events (server components broadcast to multiple subscribers)
    outbound_tx: broadcast::Sender<StatusEvent>,

    // === INBOUND (Client → Server) ===
    /// Unbounded sender for inbound events (WebSocket sends to server components)
    inbound_tx: mpsc::UnboundedSender<StatusEvent>,
    /// Unbounded receiver for inbound events (wrapped in Arc<Mutex> for shared access)
    inbound_rx: Arc<Mutex<mpsc::UnboundedReceiver<StatusEvent>>>,

    /// Targeted waiters for specific events (e.g., HITL decisions)
    /// Map: event_key → oneshot sender for direct event routing
    event_waiters: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<StatusEvent>>>>,
}

impl fmt::Debug for BidirectionalEventChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BidirectionalEventChannel")
            .field("id", &self.id)
            .finish()
    }
}

impl BidirectionalEventChannel {
    /// Create a new bidirectional event channel
    pub fn new(id: String) -> Self {
        // Outbound: broadcast channel with buffer for multiple subscribers (server → client)
        let (outbound_tx, _) = broadcast::channel(1000);

        // Inbound: mpsc unbounded channel for client messages (client → server)
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();

        Self {
            id,
            outbound_tx,
            inbound_tx,
            inbound_rx: Arc::new(Mutex::new(inbound_rx)),
            event_waiters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Send outbound event (server → client)
    /// This broadcasts the event to all connected WebSocket subscribers
    pub async fn send(&self, event: StatusEvent) -> Result<()> {
        info!("📤 Broadcasting event to channel {}: {:?}", self.id, event.event);

        // Broadcast to all subscribers (ignore errors if no receivers)
        match self.outbound_tx.send(event) {
            Ok(_) => Ok(()),
            Err(e) => {
                debug!("No active receivers for channel {} (this is OK if WebSocket not connected yet)", self.id);
                Ok(()) // Not an error - WebSocket might connect later
            }
        }
    }

    /// Subscribe to outbound events (server → client)
    /// WebSocket handler calls this to receive events
    pub fn subscribe_outbound(&self) -> broadcast::Receiver<StatusEvent> {
        info!("🔌 WebSocket subscribing to outbound events for channel {}", self.id);
        self.outbound_tx.subscribe()
    }

    /// Send inbound event (client → server)
    /// WebSocket handler calls this when receiving messages from client
    pub async fn receive_inbound(&self, event: StatusEvent) -> Result<()> {
        info!("📥 Received inbound event on channel {}: {:?}", self.id, event.event);

        // Send to inbound channel
        self.inbound_tx.send(event.clone())
            .map_err(|_| anyhow::anyhow!("Inbound channel closed for {}", self.id))?;

        // Check for targeted waiters (e.g., HITL decisions waiting for response)
        if let Some(key) = Self::extract_event_key(&event) {
            let mut waiters = self.event_waiters.lock().await;
            if let Some(tx) = waiters.remove(&key) {
                info!("✅ Routing targeted event {} to waiting agent", key);
                let _ = tx.send(event);
            }
        }

        Ok(())
    }

    /// Wait for specific inbound event with timeout (client → server)
    /// Agents call this when they need human approval (HITL)
    pub async fn wait_for(&self, event_key: String, timeout: Duration) -> Result<StatusEvent> {
        info!("⏳ Agent waiting for targeted event: {} (timeout: {:?})", event_key, timeout);

        let (tx, rx) = tokio::sync::oneshot::channel();

        // Register waiter
        {
            let mut waiters = self.event_waiters.lock().await;
            waiters.insert(event_key.clone(), tx);
        }

        // Wait with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(event)) => {
                info!("✅ Received targeted event: {}", event_key);
                Ok(event)
            }
            Ok(Err(_)) => Err(anyhow::anyhow!("Event waiter channel closed for {}", event_key)),
            Err(_) => {
                // Cleanup on timeout
                let mut waiters = self.event_waiters.lock().await;
                waiters.remove(&event_key);
                Err(anyhow::anyhow!("Timeout waiting for event: {}", event_key))
            }
        }
    }

    /// Try receive next inbound event (non-blocking)
    pub async fn try_recv(&self) -> Result<Option<StatusEvent>> {
        let mut rx = self.inbound_rx.lock().await;
        Ok(rx.try_recv().ok())
    }

    /// Receive next inbound event (blocking)
    pub async fn recv(&self) -> Result<StatusEvent> {
        let mut rx = self.inbound_rx.lock().await;
        rx.recv().await
            .ok_or_else(|| anyhow::anyhow!("Inbound channel closed for {}", self.id))
    }

    /// Extract event key for targeted routing
    fn extract_event_key(event: &StatusEvent) -> Option<String> {
        match &event.event {
            EventType::HitlDecision { id, .. } => {
                Some(format!("hitl_decision:{}", id))
            }
            _ => None,
        }
    }

    /// Get channel ID
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Subscription for a client session
/// Each subscription maintains a bidirectional channel, connection state, and event buffer
pub struct Subscription {
    /// Unique subscription ID
    pub id: String,

    /// Optional client identifier
    pub client_id: Option<String>,

    /// When subscription was created
    pub created_at: DateTime<Utc>,

    /// When subscription expires
    pub expires_at: DateTime<Utc>,

    /// Last activity time (connection, disconnection, event)
    pub last_activity: DateTime<Utc>,

    /// Whether WebSocket is currently connected
    pub connected: bool,

    /// The bidirectional event channel for this subscription
    /// This is the SINGLE source of truth for all communication
    pub channel: BidirectionalEventChannel,
}

impl Subscription {
    pub fn new(client_id: Option<String>, ttl_minutes: i64) -> Self {
        let id = format!("sub_{}", Uuid::new_v4());
        let now = Utc::now();
        let expires_at = now + chrono::Duration::minutes(ttl_minutes);

        // Create bidirectional channel for this subscription
        let channel = BidirectionalEventChannel::new(id.clone());

        Self {
            id,
            client_id,
            created_at: now,
            expires_at,
            last_activity: now,
            connected: false,
            channel,
        }
    }

    /// Mark WebSocket as connected
    pub fn connect(&mut self) {
        self.connected = true;
        self.last_activity = Utc::now();
        info!("🔌 WebSocket connected for subscription {}", self.id);
    }

    /// Mark WebSocket as disconnected
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.last_activity = Utc::now();
        info!("🔌 WebSocket disconnected for subscription {}", self.id);
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }

    /// Check if subscription has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if inactive
    pub fn is_inactive(&self, max_inactivity: chrono::Duration) -> bool {
        Utc::now() - self.last_activity > max_inactivity
    }

    /// Receive event
    pub async fn receive_inbound(&self, event: StatusEvent) -> Result<()>{
        self.channel.receive_inbound(event).await
    }
}

/// Subscription status information for API responses
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubscriptionStatusInfo {
    pub subscription_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub connected: bool,
    pub client_id: Option<String>,
}

/// Execution Manager - manages subscriptions and orchestrator lifecycle
pub struct ExecutionManager {
    /// Active subscriptions by ID
    pub subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,

    /// Agent pool
    agent_pool: Arc<AgentPool>,

    /// Coordination manager
    coordination_manager: Arc<CoordinationManager>,

    /// File lock manager
    file_lock_manager: Arc<FileLockManager>,

    /// RAG system
    rag: Arc<SmartMultiSourceRag>,

    /// History manager
    history_manager: Arc<RwLock<HistoryManager>>,

    /// System config
    config: Arc<SystemConfig>,

    /// Subscription TTL in minutes
    subscription_ttl: i64,

    /// Last cleanup time
    last_cleanup: Arc<Mutex<Instant>>,

    shared_context: Arc<RwLock<SharedContext>>,

    audit_logger: Arc<AuditLogger>,

    embedding_client: Arc<EmbeddingClient>,
}

impl ExecutionManager {
    #[instrument(name = "execution_manager_init", skip(config), fields(agents = config.agent_network.agents.len()))]
    pub async fn new(config: SystemConfig) -> Result<Self> {
        info!("Initializing ExecutionManager");

        // Initialize all components
        let agent_pool = Arc::new(AgentPool::new(&config).await?);
        let shared_context = Arc::new(RwLock::new(SharedContext::new()));
        let coordination_manager = Arc::new(CoordinationManager::new());
        let file_lock_manager = Arc::new(FileLockManager::new(30));

        // HITL setup
        let audit_logger = Arc::new(AuditLogger);

        // RAG and history setup
        let embedding_client = Arc::new(EmbeddingClient::new(
            &config.embedding.dense_model,
            config.embedding.vector_size
        )?);

        let rag = SmartMultiSourceRag::new(&config, embedding_client.clone()).await?;
        let history_manager = Arc::new(RwLock::new(
            HistoryManager::new(&config.storage.postgres_url, &config.rag).await?
        ));

        let last_cleanup = Arc::new(Mutex::new(Instant::now()));

        info!("ExecutionManager initialized successfully");

        Ok(Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(config),
            agent_pool,
            shared_context,
            coordination_manager,
            file_lock_manager,
            audit_logger,
            rag,
            history_manager,
            embedding_client,
            subscription_ttl: 500,
            last_cleanup,
        })
    }

    /// Create new subscription and return the subscription ID
    #[instrument(skip(self), fields(client_id = ?client_id))]
    pub async fn create_subscription(&self, client_id: Option<String>) -> Result<String> {
        let mut subscriptions = self.subscriptions.write().await;
        let subscription = Subscription::new(client_id.clone(), self.subscription_ttl);
        let subscription_id = subscription.id.clone();

        subscriptions.insert(subscription_id.clone(), subscription);
        info!("✅ Created subscription: {} (client_id: {:?})", subscription_id, client_id);

        Ok(subscription_id)
    }

    /// Get the bidirectional channel for a subscription
    /// This is what agents/orchestrator use to send events
    pub async fn get_channel(&self, subscription_id: &str) -> Result<BidirectionalEventChannel> {
        let subscriptions = self.subscriptions.read().await;
        let subscription = subscriptions.get(subscription_id)
            .ok_or_else(|| anyhow::anyhow!("Subscription not found: {}", subscription_id))?;

        Ok(subscription.channel.clone())
    }

    /// Connect WebSocket to subscription and get receiver for outbound events
    /// WebSocket handler calls this when a client connects
    pub async fn connect_subscription(&self, subscription_id: &str) -> Option<broadcast::Receiver<StatusEvent>> {
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(subscription) = subscriptions.get_mut(subscription_id) {
            subscription.connect();
            Some(subscription.channel.subscribe_outbound())
        } else {
            warn!("❌ Subscription not found: {}", subscription_id);
            None
        }
    }

    /// Disconnect from subscription
    /// WebSocket handler calls this when a client disconnects
    pub async fn disconnect_subscription(&self, subscription_id: &str) {
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(subscription) = subscriptions.get_mut(subscription_id) {
            subscription.disconnect();
        }
    }

    /// Get subscription info
    pub async fn get_subscription_info(&self, subscription_id: &str) -> Option<SubscriptionStatusInfo> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.get(subscription_id).map(|sub| SubscriptionStatusInfo {
            subscription_id: sub.id.clone(),
            status: if sub.connected { "connected".to_string() } else { "disconnected".to_string() },
            created_at: sub.created_at,
            expires_at: sub.expires_at,
            connected: sub.connected,
            client_id: sub.client_id.clone(),
        })
    }

    /// Execute query asynchronously
    /// This creates a background task that uses the subscription's bidirectional channel
    #[instrument(skip(self, query), fields(query_len = query.len()))]
    pub async fn execute_query(
        &self,
        query: &String,
        project_scope: ProjectScope,
        subscription_id: &String,
    ) -> Result<()> {
        // Get the bidirectional channel for this subscription
        let event_channel = self.get_channel(subscription_id).await?;
        let conversation_id = ConversationId::new();

        info!("🚀 Starting query execution for subscription {}", subscription_id);

        // Send execution started event
        event_channel.send(StatusEvent {
            conversation_id: conversation_id.to_string(),
            timestamp: Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::ExecutionStarted {
                query: query.clone(),
            },
        }).await?;

        // Clone all necessary data for the background task
        let query_clone = query.clone();
        let project_scope_clone = project_scope.clone();
        let conversation_id_clone = conversation_id.clone();
        let event_channel_clone = event_channel.clone();
        let config_clone = self.config.clone();
        let agent_pool_clone = self.agent_pool.clone();
        let shared_context_clone = self.shared_context.clone();
        let coordination_manager_clone = self.coordination_manager.clone();
        let file_lock_manager_clone = self.file_lock_manager.clone();
        let audit_logger_clone = self.audit_logger.clone();
        let rag_clone = self.rag.clone();
        let history_manager_clone = self.history_manager.clone();
        let embedding_client_clone = self.embedding_client.clone();

        // Execute in background task
        tokio::spawn(async move {
            info!("🔄 Background task started for conversation {}", conversation_id);

            match Orchestrator::execute_query(
                &query_clone,
                project_scope_clone,
                conversation_id_clone,
                event_channel_clone.clone(),
                config_clone,
                agent_pool_clone,
                shared_context_clone,
                coordination_manager_clone,
                file_lock_manager_clone,
                audit_logger_clone,
                rag_clone,
                history_manager_clone,
                embedding_client_clone
            ).await {
                Ok(result) => {
                    info!("✅ Query execution completed successfully");
                    let _ = event_channel_clone.send(StatusEvent {
                        conversation_id: conversation_id.to_string(),
                        timestamp: Utc::now(),
                        source: EventSource::Orchestrator,
                        event: EventType::ExecutionCompleted {
                            result: result.clone(),
                        },
                    }).await;
                    Ok(result)
                }
                Err(e) => {
                    error!("❌ Query execution failed: {}", e);
                    let _ = event_channel_clone.send(StatusEvent {
                        conversation_id: conversation_id.to_string(),
                        timestamp: Utc::now(),
                        source: EventSource::Orchestrator,
                        event: EventType::ExecutionFailed {
                            error: e.to_string(),
                        },
                    }).await;
                    Err(e)
                }
            }
        });

        Ok(())
    }

    /// Cleanup expired subscriptions
    pub async fn cleanup_expired_subscriptions(&self) -> usize {
        let mut last_cleanup = self.last_cleanup.lock().await;

        // Only cleanup every 5 minutes
        if last_cleanup.elapsed() < Duration::from_secs(300) {
            return 0;
        }

        let mut subscriptions = self.subscriptions.write().await;
        let initial_count = subscriptions.len();

        subscriptions.retain(|id, sub| {
            let should_keep = !sub.is_expired() &&
                             (sub.connected || !sub.is_inactive(chrono::Duration::hours(1)));

            if !should_keep {
                info!("🧹 Cleaning up subscription: {} (expired={}, inactive={})",
                      id, sub.is_expired(), sub.is_inactive(chrono::Duration::hours(1)));
            }

            should_keep
        });

        *last_cleanup = Instant::now();
        let removed = initial_count - subscriptions.len();

        if removed > 0 {
            info!("🧹 Cleaned up {} expired/inactive subscriptions", removed);
        }

        removed
    }
}
