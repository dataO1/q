//! Execution Manager - handles conversation streams and orchestrator lifecycle
//!
//! The ExecutionManager provides:
//! - Bidirectional status stream management using Tokio channels
//! - Async execution of queries via stateless Orchestrator
//! - Session lifecycle and cleanup
//! - Generic StatusEvent routing (server ↔ client)

use std::collections::{HashMap, VecDeque};
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

/// Subscription for bidirectional buffered event streaming
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

    // === OUTBOUND (Server → Client) ===
    /// Broadcast channel for outbound events (1-to-many)
    pub outbound_tx: broadcast::Sender<StatusEvent>,

    /// Buffer of events for replay on reconnect
    pub outbound_buffer: VecDeque<StatusEvent>,

    // === INBOUND (Client → Server) ===
    /// Unbounded channel for inbound events (many-to-1)
    /// Multiple WebSocket handlers can send, orchestrator/agents receive
    pub inbound_tx: mpsc::UnboundedSender<StatusEvent>,
    pub inbound_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<StatusEvent>>>,

    /// Targeted waiters for specific events (e.g., HITL decisions)
    /// Map: event_key → oneshot sender
    pub event_waiters: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<StatusEvent>>>>,
}

impl Subscription {
    pub fn new(client_id: Option<String>, ttl_minutes: i64) -> Self {
        let id = format!("sub_{}", Uuid::new_v4());
        let now = Utc::now();
        let expires_at = now + chrono::Duration::minutes(ttl_minutes);

        // Outbound: broadcast channel (server → client)
        let (outbound_tx, _) = broadcast::channel(1000);

        // Inbound: mpsc channel (client → server)
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();

        Self {
            id,
            client_id,
            created_at: now,
            expires_at,
            last_activity: now,
            connected: false,
            outbound_tx,
            outbound_buffer: VecDeque::new(),
            inbound_tx,
            inbound_rx: Arc::new(tokio::sync::Mutex::new(inbound_rx)),
            event_waiters: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Send outbound event (server → client)
    pub fn send_outbound(&mut self, event: StatusEvent) {
        self.last_activity = Utc::now();

        // Buffer event for replay
        self.outbound_buffer.push_back(event.clone());
        if self.outbound_buffer.len() > 500 {
            self.outbound_buffer.pop_front();
        }

        // Broadcast to connected clients (ignore errors if no receivers)
        let _ = self.outbound_tx.send(event);
    }

    /// Receive inbound event (client → server)
    pub async fn receive_inbound(&self, event: StatusEvent) -> Result<()> {
        self.inbound_tx.send(event.clone())
            .map_err(|_| anyhow::anyhow!("Inbound channel closed"))?;

        // Check for targeted waiters
        if let Some(key) = Self::extract_event_key(&event) {
            let mut waiters = self.event_waiters.lock().await;
            if let Some(tx) = waiters.remove(&key) {
                let _ = tx.send(event);
            }
        }

        Ok(())
    }

    /// Connect WebSocket and get outbound receiver (with buffered replay)
    pub fn connect(&mut self) -> broadcast::Receiver<StatusEvent> {
        self.connected = true;
        self.last_activity = Utc::now();

        let rx = self.outbound_tx.subscribe();

        // Replay buffered events
        for event in &self.outbound_buffer {
            let _ = self.outbound_tx.send(event.clone());
        }

        rx
    }

    /// Disconnect WebSocket
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.last_activity = Utc::now();
    }

    /// Get inbound sender clone (for WebSocket handler)
    pub fn get_inbound_sender(&self) -> mpsc::UnboundedSender<StatusEvent> {
        self.inbound_tx.clone()
    }

    /// Register waiter for specific event
    pub async fn wait_for_event(&self, event_key: String, timeout: Duration) -> Result<StatusEvent> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        {
            let mut waiters = self.event_waiters.lock().await;
            waiters.insert(event_key.clone(), tx);
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(event)) => Ok(event),
            Ok(Err(_)) => Err(anyhow::anyhow!("Event waiter channel closed")),
            Err(_) => {
                // Cleanup on timeout
                let mut waiters = self.event_waiters.lock().await;
                waiters.remove(&event_key);
                Err(anyhow::anyhow!("Timeout waiting for event: {}", event_key))
            }
        }
    }

    /// Extract event key for routing
    fn extract_event_key(event: &StatusEvent) -> Option<String> {
        match &event.event {
            EventType::HitlDecision { id, .. } => {
                Some(format!("hitl_decision:{}", id))
            }
            // EventType::ClientCommand { command_id, .. } => {
            //     Some(format!("client_command:{}", command_id))
            // }
            // EventType::UserFeedback { feedback_id, .. } => {
            //     Some(format!("user_feedback:{}", feedback_id))
            // }
            _ => None,
        }
    }

    /// Check if subscription has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if inactive
    pub fn is_inactive(&self, max_inactivity: chrono::Duration) -> bool {
        Utc::now() - self.last_activity > max_inactivity
    }
}

/// Subscription status information for API responses
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubscriptionStatusInfo {
    pub subscription_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub buffered_events: usize,
    pub connected: bool,
    pub client_id: Option<String>,
}

/// Bidirectional event channel for agents/orchestrator
#[derive(Clone)]
pub struct BidirectionalEventChannel {
    subscription_id: String,
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
}
impl fmt::Debug for BidirectionalEventChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BidirectionalEventChannel")
            .field("subscription_id", &self.subscription_id)
            .field("subscriptions", &"<Arc<RwLock<HashMap<String, Subscription>>>>")
            .finish()
    }
}

impl BidirectionalEventChannel {
    pub fn new(subscription_id: String, subscriptions: Arc<RwLock<HashMap<String, Subscription>>>) -> Self {
        Self {
            subscription_id,
            subscriptions,
        }
    }

    /// Send outbound event (server → client)
    pub async fn send(&self, event: StatusEvent) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().await;
        if let Some(subscription) = subscriptions.get_mut(&self.subscription_id) {
            subscription.send_outbound(event);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Subscription not found: {}", self.subscription_id))
        }
    }

    /// Wait for specific inbound event with timeout (client → server)
    pub async fn wait_for(&self, event_key: String, timeout: Duration) -> Result<StatusEvent> {
        let subscriptions = self.subscriptions.read().await;
        let subscription = subscriptions.get(&self.subscription_id)
            .ok_or_else(|| anyhow::anyhow!("Subscription not found: {}", self.subscription_id))?;

        // Clone Arc and drop lock before waiting
        let wait_future = subscription.wait_for_event(event_key, timeout);
        drop(subscription);

        wait_future.await
    }

    /// Try receive next inbound event (non-blocking)
    pub async fn try_recv(&self) -> Result<Option<StatusEvent>> {
        let subscriptions = self.subscriptions.read().await;
        let subscription = subscriptions.get(&self.subscription_id)
            .ok_or_else(|| anyhow::anyhow!("Subscription not found: {}", self.subscription_id))?;

        let mut rx = subscription.inbound_rx.lock().await;
        Ok(rx.try_recv().ok())
    }

    /// Receive next inbound event (blocking)
    pub async fn recv(&self) -> Result<StatusEvent> {
        let subscriptions = self.subscriptions.read().await;
        let subscription = subscriptions.get(&self.subscription_id)
            .ok_or_else(|| anyhow::anyhow!("Subscription not found: {}", self.subscription_id))?;

        let mut rx = subscription.inbound_rx.lock().await;
        rx.recv().await
            .ok_or_else(|| anyhow::anyhow!("Inbound channel closed"))
    }
}

/// Execution Manager - manages subscriptions and orchestrator lifecycle
pub struct ExecutionManager {
    /// Active subscriptions by ID
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,

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
    last_cleanup: Arc<tokio::sync::Mutex<Instant>>,
    shared_context: Arc<RwLock<SharedContext>>,
    audit_logger: Arc<AuditLogger>,
    embedding_client: Arc<ai_agent_common::llm::EmbeddingClient>,
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
        let hitl_mode = config.agent_network.hitl.mode;
        let risk_threshold = config.agent_network.hitl.risk_threshold;
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

    /// Create new subscription
    #[instrument(skip(self), fields(client_id = ?client_id))]
    pub async fn create_subscription(&self, client_id: Option<String>) -> Result<String> {
        let mut subscriptions = self.subscriptions.write().await;

        let subscription = Subscription::new(client_id.clone(), self.subscription_ttl);
        let subscription_id = subscription.id.clone();

        subscriptions.insert(subscription_id.clone(), subscription);

        info!("Created subscription: {} (client_id: {:?})", subscription_id, client_id);
        Ok(subscription_id)
    }

    /// Connect to existing subscription (WebSocket connection)
    pub async fn connect_subscription(&self, subscription_id: &str) -> Option<broadcast::Receiver<StatusEvent>> {
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(subscription) = subscriptions.get_mut(subscription_id) {
            info!("WebSocket connected to subscription: {}", subscription_id);
            Some(subscription.connect())
        } else {
            warn!("Subscription not found: {}", subscription_id);
            None
        }
    }

    /// Get inbound sender for WebSocket handler
    pub async fn get_inbound_sender(&self, subscription_id: &str) -> Option<mpsc::UnboundedSender<StatusEvent>> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.get(subscription_id).map(|s| s.get_inbound_sender())
    }

    /// Disconnect from subscription (WebSocket disconnection)
    pub async fn disconnect_subscription(&self, subscription_id: &str) {
        let mut subscriptions = self.subscriptions.write().await;

        if let Some(subscription) = subscriptions.get_mut(subscription_id) {
            info!("WebSocket disconnected from subscription: {}", subscription_id);
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
            buffered_events: sub.outbound_buffer.len(),
            connected: sub.connected,
            client_id: sub.client_id.clone(),
        })
    }

    /// Execute query asynchronously
    #[instrument(skip(self, query), fields(query_len = query.len()))]
    pub async fn execute_query(
        &self,
        query: &String,
        project_scope: ProjectScope,
        subscription_id: &String,
    ) -> Result<()> {
        let event_channel = BidirectionalEventChannel::new(
            subscription_id.clone(),
            self.subscriptions.clone(),
        );

        let conversation_id = ConversationId::new();

        // Send execution started event
        event_channel.send(StatusEvent {
            conversation_id: conversation_id.to_string(),
            timestamp: Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::ExecutionStarted {
                query: query.clone(),
            },
        }).await?;
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
            match Orchestrator::execute_query(&query_clone, project_scope_clone, conversation_id_clone, event_channel_clone, config_clone, agent_pool_clone, shared_context_clone, coordination_manager_clone, file_lock_manager_clone, audit_logger_clone, rag_clone, history_manager_clone, embedding_client_clone).await {
                Ok(result) => {
                    let _ = event_channel.send(StatusEvent {
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
                    error!("Query execution failed: {}", e);
                    let _ = event_channel.send(StatusEvent {
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
                info!("Cleaning up subscription: {} (expired={}, inactive={})",
                    id, sub.is_expired(), sub.is_inactive(chrono::Duration::hours(1)));
            }

            should_keep
        });

        *last_cleanup = Instant::now();
        let removed = initial_count - subscriptions.len();

        if removed > 0 {
            info!("Cleaned up {} expired/inactive subscriptions", removed);
        }

        removed
    }
}
