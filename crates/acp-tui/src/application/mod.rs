//! Application layer - Core Elm architecture implementation
//!
//! This module implements the Model-Update-View pattern from Elm architecture.

pub mod state;
pub mod update;
pub mod view;

pub use state::AppModel;
pub use update::update_app;
pub use view::render_app;

use std::{io, sync::Arc, time::Duration};
use anyhow::{Context, Result};
use crossterm::{execute, terminal::{disable_raw_mode, LeaveAlternateScreen}};
use tokio::{sync::mpsc, time::interval, signal};
use tracing::{info, warn, error, instrument};
use tuirealm::{
    application::PollStrategy, terminal::{TerminalBridge, CrosstermTerminalAdapter},
    Application as TuiApplication, EventListenerCfg,
};

use crate::{
    message::{AppMsg, ComponentId, NoUserEvent},
    components::realm::{
        TimelineRealmComponent, QueryInputRealmComponent, StatusLineRealmComponent,
        HitlReviewRealmComponent, HitlQueueRealmComponent,
    },
    client::AcpClient,
    services::{ApiService, QueryExecutor, WebSocketManager},
    utils::generate_client_id,
    config::Config,
};

/// Main application following Elm architecture
pub struct Application {
    /// TUIRealm application for UI components
    ui_app: TuiApplication<ComponentId, AppMsg, NoUserEvent>,
    /// Terminal interface
    terminal: TerminalBridge<CrosstermTerminalAdapter>,
    /// Application model (state)
    model: AppModel,
    /// Message sender for async operations
    sender: mpsc::UnboundedSender<AppMsg>,
    /// Message receiver
    receiver: mpsc::UnboundedReceiver<AppMsg>,
    /// Services
    api_service: ApiService,
    query_executor: QueryExecutor,
    websocket_manager: WebSocketManager,
    /// Animation timer
    animation_timer: tokio::time::Interval,
    /// Whether the UI needs to be rerendered
    needs_render: bool,
}

impl Application {
    /// Create new application instance
    #[instrument(skip(config))]
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing Elm-based ACP TUI application");
        
        // Generate client ID
        let client_id = generate_client_id()?;
        info!("Generated client ID: {}", client_id);
        
        // Initialize model
        let model = AppModel::new(client_id.clone());
        
        // Initialize terminal
        let terminal = TerminalBridge::init_crossterm()
            .context("Failed to initialize terminal")?;
        
        // Create message channel
        let (sender, receiver) = mpsc::unbounded_channel();
        
        // Initialize TUIRealm application
        let mut ui_app = TuiApplication::init(
            EventListenerCfg::default()
                .crossterm_input_listener(Duration::from_millis(20), 1000) // Duration + max_poll
        );
        
        // Mount all components  
        ui_app.mount(ComponentId::Timeline, Box::new(TimelineRealmComponent::new()), vec![])
            .context("Failed to mount Timeline component")?;
        
        ui_app.mount(ComponentId::QueryInput, Box::new(QueryInputRealmComponent::new()), vec![])
            .context("Failed to mount QueryInput component")?;
        
        ui_app.mount(ComponentId::StatusLine, Box::new(StatusLineRealmComponent::new()), vec![])
            .context("Failed to mount StatusLine component")?;
        
        ui_app.mount(ComponentId::HitlReview, Box::new(HitlReviewRealmComponent::new()), vec![])
            .context("Failed to mount HitlReview component")?;
        
        ui_app.mount(ComponentId::HitlQueue, Box::new(HitlQueueRealmComponent::new()), vec![])
            .context("Failed to mount HitlQueue component")?;
        
        // Set initial focus
        ui_app.active(&ComponentId::QueryInput)
            .context("Failed to set initial focus")?;
        
        // Initialize services
        let acp_client = Arc::new(AcpClient::new(&config.server_url)?);
        let api_service = ApiService::new(acp_client.clone());
        let query_executor = QueryExecutor::new(api_service.clone(), sender.clone());
        let websocket_manager = WebSocketManager::new(config.server_url.clone(), sender.clone());
        
        // Animation timer - configurable rate for smooth animations when needed
        let animation_timer = interval(Duration::from_millis(config.ui.animation_interval_ms));
        
        info!("Application initialized successfully");
        
        Ok(Self {
            ui_app,
            terminal,
            model,
            sender,
            receiver,
            api_service,
            query_executor,
            websocket_manager,
            animation_timer,
            needs_render: true, // Initial render is needed
        })
    }
    
    /// Run the main application loop
    #[instrument(skip(self))]
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting application main loop");
        
        // Initialize WebSocket connection
        if let Err(e) = self.websocket_manager.connect().await {
            warn!("Failed to connect WebSocket: {}", e);
        }
        
        // Set up signal handling for graceful shutdown  
        let ctrl_c = signal::ctrl_c();
        tokio::pin!(ctrl_c);

        'main_loop: loop {
            tokio::select! {
                // Handle UI events through TUIRealm with adaptive polling
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    // Use UpTo strategy for more efficient polling
                    if let Ok(messages) = self.ui_app.tick(PollStrategy::Once) {
                        for msg in messages {
                            if self.handle_message(msg).await? {
                                break 'main_loop;
                            }
                        }
                    }
                },
                
                // Handle internal messages with batching
                msg = self.receiver.recv() => {
                    if let Some(msg) = msg {
                        // Collect additional messages if available (batching)
                        let mut messages = vec![msg];
                        while let Ok(additional_msg) = self.receiver.try_recv() {
                            messages.push(additional_msg);
                            // Limit batch size to prevent blocking
                            if messages.len() >= 10 {
                                break;
                            }
                        }
                        
                        // Process all batched messages
                        for batched_msg in messages {
                            if self.handle_message(batched_msg).await? {
                                break 'main_loop;
                            }
                        }
                    }
                },
                
                // Animation timer - only send tick if animations are active
                _ = self.animation_timer.tick() => {
                    // Check if we have any active animations before sending tick
                    if self.has_active_animations() {
                        let _ = self.sender.send(AppMsg::Tick);
                    }
                },
                
                // Handle Ctrl+C (cross-platform)
                _ = &mut ctrl_c => {
                    info!("Received Ctrl+C, initiating graceful shutdown");
                    break 'main_loop;
                }
            }
            
            // Only render the UI if something has changed
            if self.needs_render || self.model.component_dirty_flags.any_dirty() {
                self.render();
                self.needs_render = false;
                // Clear dirty flags after rendering
                self.model.component_dirty_flags.clear_all();
            }
        }
        
        // Shutdown sequence - called after main loop exits
        info!("Initiating graceful shutdown");
        
        // Disconnect WebSocket
        if let Err(e) = self.websocket_manager.disconnect().await {
            warn!("Error during WebSocket disconnection: {}", e);
        }
        
        // Wait a brief moment for any final operations
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Restore terminal
        if let Err(e) = disable_raw_mode() {
            error!("Failed to disable raw mode: {}", e);
        }
        
        if let Err(e) = execute!(io::stdout(), LeaveAlternateScreen) {
            error!("Failed to leave alternate screen: {}", e);
        }
        
        info!("Graceful shutdown complete");
        Ok(())
    }
    
    /// Handle a message using the Elm update pattern
    async fn handle_message(&mut self, msg: AppMsg) -> Result<bool> {
        // Check for quit message first
        if matches!(msg, AppMsg::Quit) {
            return Ok(true);
        }
        
        // Handle side effects first
        self.handle_side_effects(&msg).await?;
        
        // Update model with Elm update function
        let effects = update_app(&mut self.model, msg)?;
        
        // Mark that we need to render since the model has changed
        self.needs_render = true;
        
        // Handle any effects generated by the update
        for effect in effects {
            self.handle_side_effects(&effect).await?;
            let additional_effects = update_app(&mut self.model, effect)?;
            // Could recursively handle additional effects if needed
            for additional_effect in additional_effects {
                let _ = self.sender.send(additional_effect);
            }
        }
        
        // Update UI components with new model state
        self.sync_ui_with_model().await?;
        
        Ok(false)
    }
    
    /// Handle side effects (I/O operations)
    async fn handle_side_effects(&mut self, msg: &AppMsg) -> Result<()> {
        match msg {
            AppMsg::QuerySubmitted => {
                if !self.model.query_text.trim().is_empty() {
                    let query = self.model.query_text.clone();
                    let executor = self.query_executor.clone();
                    tokio::spawn(async move {
                        let _ = executor.execute_query(query).await;
                    });
                }
            }
            
            AppMsg::HitlDecisionMade(request_id, decision) => {
                // Submit HITL decision to API
                let request_id = request_id.clone();
                let decision = decision.clone();
                let api_service = self.api_service.clone();
                let sender = self.sender.clone();
                
                tokio::spawn(async move {
                    match api_service.submit_hitl_decision(request_id.clone(), decision).await {
                        Ok(_) => {
                            let _ = sender.send(AppMsg::HitlDecisionSent(request_id));
                        }
                        Err(e) => {
                            let _ = sender.send(AppMsg::HitlDecisionFailed(request_id, e.to_string()));
                        }
                    }
                });
            }
            
            AppMsg::WebSocketDisconnected => {
                // Attempt to reconnect
                if let Err(e) = self.websocket_manager.reconnect().await {
                    error!("Failed to reconnect WebSocket: {}", e);
                }
            }
            
            _ => {} // No side effects for other messages
        }
        
        Ok(())
    }
    
    /// Sync UI components with model state
    async fn sync_ui_with_model(&mut self) -> Result<()> {
        // Update query input text
        let _ = self.ui_app.attr(
            &ComponentId::QueryInput,
            tuirealm::Attribute::Text,
            tuirealm::AttrValue::String(self.model.query_text.clone()),
        );
        
        // Update focus
        if let Err(e) = self.ui_app.active(&self.model.focused_component) {
            tracing::warn!("Failed to set focus to component {:?}: {}", self.model.focused_component, e);
        }
        
        // Sync other component states as needed
        
        Ok(())
    }
    
    /// Render the application
    fn render(&mut self) {
        let model = &self.model;
        let _ = self.terminal.draw(|frame| {
            render_app(model, &mut self.ui_app, frame);
        });
    }
    
    /// Cleanup resources
    #[instrument(skip(self))]
    pub fn cleanup(self) -> Result<()> {
        info!("Cleaning up application resources");
        
        // Disconnect WebSocket
        let mut websocket_manager = self.websocket_manager;
        tokio::spawn(async move {
            let _ = websocket_manager.disconnect().await;
        });
        
        // Restore terminal
        disable_raw_mode().context("Failed to disable raw mode")?;
        execute!(io::stdout(), LeaveAlternateScreen).context("Failed to leave alternate screen")?;
        
        info!("Application cleanup completed");
        Ok(())
    }
    
    
    /// Check if there are any active animations that need updates
    fn has_active_animations(&self) -> bool {
        // Check if timeline has active animations
        self.model.timeline_tree.get_stats().running > 0
    }
}


