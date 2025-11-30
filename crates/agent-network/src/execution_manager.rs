//! Execution Manager - handles conversation streams and orchestrator lifecycle
//!
//! The ExecutionManager provides:
//! - Conversation-level status stream management
//! - Async execution of queries via stateless Orchestrator
//! - Session lifecycle and cleanup
//! - Clean separation between streaming and business logic

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use anyhow::Result;
use tracing::{debug, info, warn, error, instrument};
use chrono::{DateTime, Utc};

use crate::agents::AgentPool;
use crate::coordination::CoordinationManager;
use crate::filelocks::FileLockManager;
use crate::hitl::{AuditLogger, DefaultApprovalQueue};
use crate::sharedcontext::SharedContext;
use crate::orchestrator::Orchestrator;

use ai_agent_common::{
    ConversationId, ProjectScope, SystemConfig, StatusEvent, EventSource, EventType,
};
use ai_agent_history::HistoryManager;
use ai_agent_rag::SmartMultiSourceRag;
use ai_agent_common::llm::EmbeddingClient;


/// Subscription for buffered event streaming
#[derive(Debug, Clone)]
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
    /// Buffer of events since subscription creation
    pub event_buffer: VecDeque<StatusEvent>,
    /// Whether WebSocket is currently connected
    pub connected: bool,
    /// Broadcast sender for this subscription
    pub sender: Arc<broadcast::Sender<StatusEvent>>,
}

impl Subscription {
    pub fn new(client_id: Option<String>, ttl_minutes: i64) -> Self {
        let id = format!("sub_{}", Uuid::new_v4());
        let now = Utc::now();
        let expires_at = now + chrono::Duration::minutes(ttl_minutes);
        let (sender, _) = broadcast::channel(1000);
        
        Self {
            id,
            client_id,
            created_at: now,
            expires_at,
            last_activity: now,
            event_buffer: VecDeque::new(),
            connected: false,
            sender: Arc::new(sender),
        }
    }
    
    /// Add event to buffer and broadcast if connected
    pub fn add_event(&mut self, event: StatusEvent) {
        // Update activity timestamp
        self.last_activity = Utc::now();
        
        // Always buffer events
        self.event_buffer.push_back(event.clone());
        
        // Limit buffer size to prevent memory issues
        if self.event_buffer.len() > 500 {
            self.event_buffer.pop_front();
        }
        
        // Broadcast if connected
        if self.connected {
            let _ = self.sender.send(event);
        }
    }
    
    /// Connect WebSocket and get receiver with replay
    pub fn connect(&mut self) -> broadcast::Receiver<StatusEvent> {
        self.connected = true;
        self.last_activity = Utc::now();
        
        // Create receiver
        let receiver = self.sender.subscribe();
        
        // Replay buffered events
        for event in &self.event_buffer {
            let _ = self.sender.send(event.clone());
        }
        
        receiver
    }
    
    /// Disconnect WebSocket (but keep subscription alive for reconnection)
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.last_activity = Utc::now();
    }
    
    /// Check if subscription has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
    
    /// Check if any query has been executed (has any events in buffer)
    pub fn has_executed_query(&self) -> bool {
        !self.event_buffer.is_empty()
    }
    
    /// Check if subscription has been inactive for too long (for cleanup)
    pub fn is_inactive(&self, max_inactivity: chrono::Duration) -> bool {
        Utc::now() - self.last_activity > max_inactivity
    }
}

/// Subscription status information for API responses
#[derive(Debug, Clone)]
pub struct SubscriptionStatusInfo {
    pub subscription_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub buffered_events: usize,
    pub connected: bool,
    pub client_id: Option<String>,
}

/// Buffered event sender that routes events through subscription buffering
#[derive(Clone, Debug)]
pub struct BufferedEventSender {
    subscription_id: String,
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
}

impl BufferedEventSender {
    /// Create a new buffered event sender for a subscription
    pub fn new(subscription_id: String, subscriptions: Arc<RwLock<HashMap<String, Subscription>>>) -> Self {
        Self {
            subscription_id,
            subscriptions,
        }
    }
    
    /// Send an event, routing it through the subscription's buffering mechanism
    pub async fn send(&self, event: StatusEvent) -> Result<(), broadcast::error::SendError<StatusEvent>> {
        let mut subscriptions = self.subscriptions.write().await;
        
        if let Some(subscription) = subscriptions.get_mut(&self.subscription_id) {
            subscription.add_event(event.clone());
            Ok(())
        } else {
            warn!("Tried to send event for unknown subscription: {}", self.subscription_id);
            Err(broadcast::error::SendError(event))
        }
    }
}

/// ExecutionManager handles subscription-based streaming and orchestrates query execution
pub struct ExecutionManager {
    /// Active subscriptions for buffered streaming
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
    
    /// System configuration
    config: Arc<SystemConfig>,
    
    /// Agent pool for task execution
    agent_pool: Arc<AgentPool>,
    
    /// Shared context across agents
    shared_context: Arc<RwLock<SharedContext>>,
    
    /// Task coordination
    coordination: Arc<CoordinationManager>,
    
    /// File lock manager
    file_locks: Arc<FileLockManager>,
    
    /// HITL components
    approval_queue: Arc<DefaultApprovalQueue>,
    audit_logger: Arc<AuditLogger>,
    
    /// RAG and history
    rag: Arc<SmartMultiSourceRag>,
    history_manager: Arc<RwLock<HistoryManager>>,
    embedding_client: Arc<EmbeddingClient>,
}

impl ExecutionManager {
    /// Create a new ExecutionManager from system configuration
    #[instrument(name = "execution_manager_init", skip(config), fields(agents = config.agent_network.agents.len()))]
    pub async fn new(config: SystemConfig) -> Result<Self> {
        info!("Initializing ExecutionManager");
        
        // Initialize all components
        let agent_pool = Arc::new(AgentPool::new(&config.agent_network.agents).await?);
        let shared_context = Arc::new(RwLock::new(SharedContext::new()));
        let coordination = Arc::new(CoordinationManager::new());
        let file_locks = Arc::new(FileLockManager::new(30));
        
        // HITL setup
        let hitl_mode = config.agent_network.hitl.mode;
        let risk_threshold = config.agent_network.hitl.risk_threshold;
        let approval_queue = Arc::new(DefaultApprovalQueue::new(hitl_mode, risk_threshold));
        let audit_logger = Arc::new(AuditLogger);
        
        // Spawn approval handler background task
        let queue_clone = Arc::clone(&approval_queue);
        let handler = Arc::new(crate::hitl::ConsoleApprovalHandler);
        tokio::spawn(async move {
            queue_clone.run_approver(handler).await;
        });
        
        // RAG and history setup
        let embedding_client = Arc::new(EmbeddingClient::new(
            &config.embedding.dense_model, 
            config.embedding.vector_size
        )?);
        
        let rag = SmartMultiSourceRag::new(&config, embedding_client.clone()).await?;
        let history_manager = Arc::new(RwLock::new(
            HistoryManager::new(&config.storage.postgres_url, &config.rag).await?
        ));
        
        info!("ExecutionManager initialized successfully");
        
        Ok(Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(config),
            agent_pool,
            shared_context,
            coordination,
            file_locks,
            approval_queue,
            audit_logger,
            rag,
            history_manager,
            embedding_client,
        })
    }
    
    /// Execute a user query with subscription_id - returns subscription_id, streams progress
    #[instrument(name = "execute_query", skip(self))]
    pub async fn execute_query(
        &self,
        query: &str,
        project_scope: ProjectScope,
        subscription_id: &str,
    ) -> Result<String> {
        info!("Processing query with subscription: {}", subscription_id);
        
        // Get subscription and mark as executed
        let mut subscriptions = self.subscriptions.write().await;
        let subscription = subscriptions.get_mut(subscription_id)
            .ok_or_else(|| anyhow::anyhow!("Subscription '{}' not found", subscription_id))?;
        
        if subscription.is_expired() {
            return Err(anyhow::anyhow!("Subscription '{}' has expired", subscription_id));
        }
        
        drop(subscriptions);
        
        // Generate a unique execution ID for this query
        let execution_id = Uuid::new_v4().to_string();
        
        // Emit execution started event
        let start_event = StatusEvent {
            execution_id: execution_id.clone(),
            timestamp: chrono::Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::ExecutionStarted { 
                query: query.to_string() 
            },
        };
        self.emit_subscription_event(subscription_id, start_event).await;
        
        // Clone data for background execution
        let query_clone = query.to_string();
        let project_scope_clone = project_scope.clone();
        let subscription_id_clone = subscription_id.to_string();
        let execution_id_clone = execution_id.clone();
        
        // Clone all components for background task
        let config = Arc::clone(&self.config);
        let agent_pool = Arc::clone(&self.agent_pool);
        let shared_context = Arc::clone(&self.shared_context);
        let coordination = Arc::clone(&self.coordination);
        let file_locks = Arc::clone(&self.file_locks);
        let approval_queue = Arc::clone(&self.approval_queue);
        let audit_logger = Arc::clone(&self.audit_logger);
        let rag = Arc::clone(&self.rag);
        let history_manager = Arc::clone(&self.history_manager);
        let embedding_client = Arc::clone(&self.embedding_client);
        let subscriptions = Arc::clone(&self.subscriptions);
        
        // Create buffered event sender for this subscription
        let event_sender = BufferedEventSender::new(subscription_id_clone.clone(), subscriptions.clone());
        
        // Generate a ConversationId for the orchestrator
        let conversation_id = ConversationId::new();
        
        // Start background execution with stateless orchestrator
        tokio::spawn(async move {
            let result = Orchestrator::execute_query(
                &query_clone,
                project_scope_clone,
                conversation_id,
                event_sender,
                config,
                agent_pool,
                shared_context,
                coordination,
                file_locks,
                approval_queue,
                audit_logger,
                rag,
                history_manager,
                embedding_client,
            ).await;
            
            // Emit completion/failure event
            let completion_event = match result {
                Ok(result) => StatusEvent {
                    execution_id: execution_id_clone.clone(),
                    timestamp: chrono::Utc::now(),
                    source: EventSource::Orchestrator,
                    event: EventType::ExecutionCompleted { result },
                },
                Err(e) => {
                    error!("Execution failed for subscription {}: {}", subscription_id_clone, e);
                    StatusEvent {
                        execution_id: execution_id_clone.clone(),
                        timestamp: chrono::Utc::now(),
                        source: EventSource::Orchestrator,
                        event: EventType::ExecutionFailed { 
                            error: e.to_string() 
                        },
                    }
                },
            };
            
            // Emit final event to subscription
            let mut subs = subscriptions.write().await;
            if let Some(subscription) = subs.get_mut(&subscription_id_clone) {
                subscription.add_event(completion_event);
            }
        });
        
        // Return subscription_id confirming execution started
        Ok(subscription_id.to_string())
    }
    
    // ============== Subscription Event Management ==============
    
    /// Emit a status event to a specific subscription
    pub async fn emit_subscription_event(&self, subscription_id: &str, event: StatusEvent) {
        let mut subscriptions = self.subscriptions.write().await;
        
        if let Some(subscription) = subscriptions.get_mut(subscription_id) {
            subscription.add_event(event);
        } else {
            warn!("Tried to emit event for unknown subscription: {}", subscription_id);
        }
    }
    
    // ============== Subscription Management ==============
    
    /// Create a new subscription or resume existing one by client_id
    pub async fn create_subscription(&self, client_id: Option<String>) -> Result<Subscription> {
        let mut subscriptions = self.subscriptions.write().await;
        
        // Check if client_id already has an active subscription to resume
        if let Some(ref cid) = client_id {
            for subscription in subscriptions.values_mut() {
                if subscription.client_id.as_ref() == Some(cid) && !subscription.is_expired() {
                    info!(
                        subscription_id = %subscription.id,
                        client_id = %cid,
                        "Resuming existing subscription for client"
                    );
                    return Ok(subscription.clone());
                }
            }
        }
        
        // Create new subscription with 5 minute TTL
        let subscription = Subscription::new(client_id.clone(), 5);
        let subscription_id = subscription.id.clone();
        
        info!(
            subscription_id = %subscription_id,
            client_id = ?client_id,
            "Created new subscription"
        );
        
        // Store subscription
        let result = subscription.clone();
        subscriptions.insert(subscription_id.clone(), subscription);
        
        Ok(result)
    }
    
    /// Get subscription and connect WebSocket for streaming
    pub async fn connect_subscription(&self, subscription_id: &str) -> Option<broadcast::Receiver<StatusEvent>> {
        let mut subscriptions = self.subscriptions.write().await;
        
        if let Some(subscription) = subscriptions.get_mut(subscription_id) {
            if subscription.is_expired() {
                warn!("Attempted to connect to expired subscription: {}", subscription_id);
                None
            } else {
                info!("Connecting WebSocket to subscription: {}", subscription_id);
                Some(subscription.connect())
            }
        } else {
            warn!("Subscription not found: {}", subscription_id);
            None
        }
    }
    
    /// Disconnect WebSocket from subscription
    pub async fn disconnect_subscription(&self, subscription_id: &str) {
        let mut subscriptions = self.subscriptions.write().await;
        
        if let Some(subscription) = subscriptions.get_mut(subscription_id) {
            info!("Disconnecting WebSocket from subscription: {}", subscription_id);
            subscription.disconnect();
        }
    }
    
    /// Get subscription status
    pub async fn get_subscription_status(&self, subscription_id: &str) -> Option<SubscriptionStatusInfo> {
        let subscriptions = self.subscriptions.read().await;
        
        if let Some(subscription) = subscriptions.get(subscription_id) {
            let status = if subscription.is_expired() {
                "expired"
            } else if subscription.connected {
                "connected"
            } else if subscription.has_executed_query() {
                "active"
            } else {
                "waiting"
            };
            
            Some(SubscriptionStatusInfo {
                subscription_id: subscription.id.clone(),
                status: status.to_string(),
                created_at: subscription.created_at,
                expires_at: subscription.expires_at,
                buffered_events: subscription.event_buffer.len(),
                connected: subscription.connected,
                client_id: subscription.client_id.clone(),
            })
        } else {
            None
        }
    }
    
    
    /// Clean up expired and inactive subscriptions
    pub async fn cleanup_expired_subscriptions(&self) {
        let mut subscriptions = self.subscriptions.write().await;
        let initial_count = subscriptions.len();
        
        // Clean up subscriptions that are expired OR inactive for >30 minutes
        let max_inactivity = chrono::Duration::minutes(30);
        
        subscriptions.retain(|subscription_id, subscription| {
            if subscription.is_expired() {
                info!("Cleaning up expired subscription: {}", subscription_id);
                false
            } else if subscription.is_inactive(max_inactivity) {
                info!(
                    subscription_id = %subscription_id,
                    client_id = ?subscription.client_id,
                    last_activity = %subscription.last_activity,
                    "Cleaning up inactive subscription"
                );
                false
            } else {
                true
            }
        });
        
        let cleaned_count = initial_count - subscriptions.len();
        if cleaned_count > 0 {
            info!("Cleaned up {} expired/inactive subscriptions", cleaned_count);
        }
    }
}