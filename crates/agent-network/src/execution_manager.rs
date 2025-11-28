//! Execution Manager - handles conversation streams and orchestrator lifecycle
//!
//! The ExecutionManager provides:
//! - Conversation-level status stream management
//! - Async execution of queries via stateless Orchestrator
//! - Session lifecycle and cleanup
//! - Clean separation between streaming and business logic

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use anyhow::Result;
use tracing::{debug, info, warn, error, instrument};

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

/// Session stream for managing conversation-level status broadcasting
#[derive(Debug, Clone)]
pub struct SessionStream {
    /// Broadcast sender for this conversation's status events
    pub sender: Arc<broadcast::Sender<StatusEvent>>,
    /// When this session was created
    pub created_at: Instant,
    /// Last activity time for TTL cleanup
    pub last_activity: Instant,
}

impl SessionStream {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000); // Bounded channel capacity
        let now = Instant::now();
        Self {
            sender: Arc::new(sender),
            created_at: now,
            last_activity: now,
        }
    }
    
    /// Create a new receiver for this stream
    pub fn subscribe(&self) -> broadcast::Receiver<StatusEvent> {
        self.sender.subscribe()
    }
    
    /// Update activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }
    
    /// Check if session has expired (inactive for more than duration)
    pub fn is_expired(&self, max_age: Duration) -> bool {
        self.last_activity.elapsed() > max_age
    }
}

/// ExecutionManager handles conversation streams and orchestrates query execution
pub struct ExecutionManager {
    /// Conversation-specific status streams
    conversation_streams: Arc<RwLock<HashMap<String, SessionStream>>>,
    
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
            conversation_streams: Arc::new(RwLock::new(HashMap::new())),
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
    
    /// Execute a user query - returns conversation_id immediately, streams progress
    #[instrument(name = "execute_query", skip(self))]
    pub async fn execute_query(
        &self,
        query: &str,
        project_scope: ProjectScope,
        conversation_id: ConversationId,
    ) -> Result<String> {
        let conversation_id_str = conversation_id.to_string();
        info!("Processing query in conversation: {}", conversation_id_str);
        
        // Ensure conversation stream exists
        self.ensure_conversation_stream(&conversation_id_str).await;
        
        // Get stream sender for this conversation
        let stream_sender = self.get_stream_sender(&conversation_id_str).await
            .ok_or_else(|| anyhow::anyhow!("Failed to get stream sender"))?;
        
        // Emit execution started event
        let start_event = StatusEvent {
            execution_id: conversation_id_str.clone(),
            timestamp: chrono::Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::ExecutionStarted { 
                query: query.to_string() 
            },
        };
        self.emit_conversation_event(&conversation_id_str, start_event).await;
        
        // Clone data for background execution
        let query_clone = query.to_string();
        let project_scope_clone = project_scope.clone();
        let conversation_id_clone = conversation_id.clone();
        let conversation_id_str_clone = conversation_id_str.clone();
        
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
        let conversation_streams = Arc::clone(&self.conversation_streams);
        
        // Start background execution with stateless orchestrator
        tokio::spawn(async move {
            let result = Orchestrator::execute_query(
                &query_clone,
                project_scope_clone,
                conversation_id_clone,
                stream_sender.clone(),
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
                    execution_id: conversation_id_str_clone.clone(),
                    timestamp: chrono::Utc::now(),
                    source: EventSource::Orchestrator,
                    event: EventType::ExecutionCompleted { result },
                },
                Err(e) => {
                    error!("Execution failed for conversation {}: {}", conversation_id_str_clone, e);
                    StatusEvent {
                        execution_id: conversation_id_str_clone.clone(),
                        timestamp: chrono::Utc::now(),
                        source: EventSource::Orchestrator,
                        event: EventType::ExecutionFailed { 
                            error: e.to_string() 
                        },
                    }
                },
            };
            
            // Emit final event to conversation stream
            let streams = conversation_streams.read().await;
            if let Some(session_stream) = streams.get(&conversation_id_str_clone) {
                if let Err(_) = session_stream.sender.send(completion_event) {
                    debug!("No subscribers for conversation {}", conversation_id_str_clone);
                }
            }
        });
        
        // Return conversation_id for client to subscribe to stream
        Ok(conversation_id_str)
    }
    
    // ============== Conversation Stream Management ==============
    
    /// Ensure a conversation stream exists
    pub async fn ensure_conversation_stream(&self, conversation_id: &str) {
        let mut streams = self.conversation_streams.write().await;
        
        if !streams.contains_key(conversation_id) {
            let session_stream = SessionStream::new();
            streams.insert(conversation_id.to_string(), session_stream);
            info!("Created conversation stream for {}", conversation_id);
        }
    }
    
    /// Get a status stream receiver for a specific conversation
    pub async fn get_conversation_stream(&self, conversation_id: &str) -> Option<broadcast::Receiver<StatusEvent>> {
        let mut streams = self.conversation_streams.write().await;
        
        if let Some(session_stream) = streams.get_mut(conversation_id) {
            session_stream.touch(); // Update activity timestamp
            Some(session_stream.subscribe())
        } else {
            warn!("Requested stream for unknown conversation: {}", conversation_id);
            None
        }
    }
    
    /// Emit a status event for a specific conversation
    pub async fn emit_conversation_event(&self, conversation_id: &str, event: StatusEvent) {
        let streams = self.conversation_streams.read().await;
        
        if let Some(session_stream) = streams.get(conversation_id) {
            if let Err(_) = session_stream.sender.send(event) {
                debug!("No subscribers for conversation {}", conversation_id);
            }
        } else {
            warn!("Tried to emit event for unknown conversation: {}", conversation_id);
        }
    }
    
    /// Clean up expired conversation streams
    pub async fn cleanup_expired_streams(&self, max_age: Duration) {
        let mut streams = self.conversation_streams.write().await;
        let initial_count = streams.len();
        
        streams.retain(|conversation_id, session| {
            if session.is_expired(max_age) {
                info!("Cleaning up expired conversation stream: {}", conversation_id);
                false
            } else {
                true
            }
        });
        
        let cleaned_count = initial_count - streams.len();
        if cleaned_count > 0 {
            info!("Cleaned up {} expired conversation streams", cleaned_count);
        }
    }
    
    /// Get conversation stream sender for internal use
    async fn get_stream_sender(&self, conversation_id: &str) -> Option<Arc<broadcast::Sender<StatusEvent>>> {
        let streams = self.conversation_streams.read().await;
        streams.get(conversation_id).map(|s| Arc::clone(&s.sender))
    }
}