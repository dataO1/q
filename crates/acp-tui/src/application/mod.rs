//! Application layer - Core Elm architecture implementation
//!
//! This module implements the Model-Update-View pattern from Elm architecture.

pub mod state;
pub mod update;
pub mod view;

pub use state::AppModel;
pub use update::update;
pub use view::render_app;

use std::{io, sync::Arc, time::{Duration, Instant}, pin::Pin};
use anyhow::{Context, Result};
use crossterm::{execute, terminal::{disable_raw_mode, LeaveAlternateScreen}};
use tokio::{sync::mpsc, time::interval, signal};
use tracing::{debug, info, trace, warn, error, instrument};
use futures::{FutureExt, Future};
use tuirealm::{
    application::PollStrategy, terminal::{TerminalBridge, CrosstermTerminalAdapter},
    Application as TuiApplication, EventListenerCfg, Event,
    listener::{AsyncPort, PollAsync, ListenerResult},
    props::{PropPayload, PropValue},
};

use crate::{
    client::AcpClient, components::realm::{
        help::HelpRealmComponent, HitlQueueRealmComponent, HitlReviewRealmComponent, QueryInputRealmComponent, StatusLineRealmComponent, TimelineRealmComponent
    }, config::Config, log_state_change, message::{AppMsg, ComponentId, ComponentMsg, NoUserEvent}, services::{ApiService, QueryExecutor, WebSocketManager}, time_operation, utils::{generate_client_id, EventLogger}
};

/// Async channel wrapper that implements PollAsync for AppMsg
struct AppMsgChannel {
    receiver: mpsc::UnboundedReceiver<AppMsg>,
}

impl AppMsgChannel {
    fn new(receiver: mpsc::UnboundedReceiver<AppMsg>) -> Self {
        Self { receiver }
    }
}

#[tuirealm::async_trait]
impl PollAsync<AppMsg> for AppMsgChannel {
    async fn poll(&mut self) -> ListenerResult<Option<Event<AppMsg>>> {
        match self.receiver.recv().await {
            Some(msg) => {
                // Wrap AppMsg in a User event for TUIRealm
                Ok(Some(Event::User(msg)))
            }
            None => Ok(None), // Channel closed
        }
    }
}

/// Main application following Elm architecture
pub struct Application {
    /// TUIRealm application for UI components
    ui_app: TuiApplication<ComponentId, ComponentMsg, AppMsg>,
    /// Terminal interface
    terminal: TerminalBridge<CrosstermTerminalAdapter>,
    /// Application model (state)
    model: AppModel,
    /// Message sender for async operations
    sender: mpsc::UnboundedSender<AppMsg>,
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

        // Initialize TUIRealm application with async port integration
        let mut ui_app = TuiApplication::init(
            EventListenerCfg::default()
                .crossterm_input_listener(Duration::from_millis(20), 100) // Responsive keyboard input
                .add_async_port(
                    Box::new(AppMsgChannel::new(receiver)),
                    Duration::from_millis(10), // Poll interval
                    100 // Max poll count
                ) // Add our async message channel
                .with_handle(tokio::runtime::Handle::current()) // Enable async runtime
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
        ui_app.mount(ComponentId::Help, Box::new(HelpRealmComponent::new()), vec![])
            .context("Failed to mount HelpRealmComponent component")?;

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
        let mut ctrl_c = Box::pin(signal::ctrl_c());

        // Force initial render to show the UI immediately
        self.render();

        'main_loop: loop {
            // Check for Ctrl+C (non-blocking)
            if ctrl_c.as_mut().now_or_never().is_some() {
                info!("Received Ctrl+C, initiating graceful shutdown");
                break 'main_loop;
            }

            // Block waiting for events with timeout
            match self.ui_app.tick(PollStrategy::UpTo(100)) {
                Ok(messages) if !messages.is_empty() => {
                    for msg in messages {
                        if self.handle_component_message(msg).await? {
                            break 'main_loop;
                        }
                    }
                }
                Ok(_) => {} // Timeout, no messages
                Err(e) => {
                    error!("Tick error: {}", e);
                }
            }

            self.render();
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

    /// Handle a ComponentMsg from TUIRealm components (keyboard events)
    #[instrument(level = "debug", skip(self), fields(
        message_type = %format!("{:?}", msg).split('(').next().unwrap_or("Unknown")
    ))]
    async fn handle_component_message(&mut self, msg: ComponentMsg) -> Result<bool> {
        trace!(?msg, "Processing ComponentMsg from keyboard/UI events");

        match msg {
            ComponentMsg::AppQuit => {
                info!("Quit message received, initiating shutdown");
                return Ok(true);
            }
            ComponentMsg::QuerySubmit => {
                // Convert to AppMsg and process through normal flow
                let app_msg = AppMsg::QuerySubmitted;
                self.handle_message(app_msg).await?;
            }
            ComponentMsg::FocusNext => {
                let app_msg = AppMsg::FocusNext;
                self.handle_message(app_msg).await?;
            }
            ComponentMsg::FocusPrevious => {
                let app_msg = AppMsg::FocusPrevious;
                self.handle_message(app_msg).await?;
            }
            ComponentMsg::HelpToggle => {
                let app_msg = AppMsg::HelpToggle;
                self.handle_message(app_msg).await?;
            }
            ComponentMsg::TimelineClear => {
                let app_msg = AppMsg::TimelineClear;
                self.handle_message(app_msg).await?;
            }
            ComponentMsg::TimelineScrollUp => {
                let app_msg = AppMsg::TimelineScrollUp;
                self.handle_message(app_msg).await?;
            }
            ComponentMsg::TimelineScrollDown => {
                let app_msg = AppMsg::TimelineScrollDown;
                self.handle_message(app_msg).await?;
            }
            ComponentMsg::HitlSubmitDecision => {
                // Handle HITL decision submission logic here
                // For now, just log
                info!("HITL decision submitted");
            }
            ComponentMsg::HitlCancelReview => {
                let app_msg = AppMsg::HitlReviewClose;
                self.handle_message(app_msg).await?;
            }
            ComponentMsg::HitlOpenReview => {
                // Get the selected request and open review
                // For now, just switch to HITL layout
                let app_msg = AppMsg::LayoutHitlReview;
                self.handle_message(app_msg).await?;
            }
            ComponentMsg::None => {
                // No action needed
            }
        }

        Ok(false)
    }

    /// Handle a message using the Elm update pattern
    #[instrument(level = "debug", skip(self), fields(
        message_type = %format!("{:?}", msg).split('(').next().unwrap_or("Unknown"),
        needs_render = self.needs_render
    ))]
    async fn handle_message(&mut self, msg: AppMsg) -> Result<bool> {
        let start_time = Instant::now();
        let model_before = self.model.clone();

        // Check for quit message first
        if matches!(msg, AppMsg::Quit) {
            info!("Quit message received, initiating shutdown");
            EventLogger::log_lifecycle_event("shutdown_requested", None);
            return Ok(true);
        }

        debug!(?msg, "Processing message");

        // Handle side effects first
        let side_effect_result = time_operation!("side_effects", {
            self.handle_side_effects(&msg).await
        });
        side_effect_result?;

        // Update model with Elm update function
        let effects = time_operation!("model_update", {
            update(&mut self.model, msg.clone())
        })?;

        // Log state changes
        log_state_change!(&model_before, &self.model, &format!("{:?}", msg));

        // Mark that we need to render since the model has changed
        self.needs_render = true;

        // Handle any effects generated by the update
        for effect in effects {
            debug!(?effect, "Processing effect message");
            self.handle_side_effects(&effect).await?;
            let additional_effects = update(&mut self.model, effect)?;
            // Could recursively handle additional effects if needed
            for additional_effect in additional_effects {
                let _ = self.sender.send(additional_effect);
            }
        }

        // Update UI components with new model state
        time_operation!("ui_sync", {
            self.sync_ui_with_model().await
        })?;

        let processing_time = start_time.elapsed();

        // Log performance metrics
        EventLogger::log_message_processing(
            &msg,
            0, // Queue size tracking removed since receiver is now in AsyncPort
            0, // Queue size tracking removed since receiver is now in AsyncPort
            Some(processing_time.as_millis()),
        );

        // Warn on slow message processing
        if processing_time.as_millis() > 50 {
            warn!(
                processing_time_ms = processing_time.as_millis(),
                message = ?msg,
                "Slow message processing detected"
            );
        }

        Ok(false)
    }

    /// Handle side effects (I/O operations)
    async fn handle_side_effects(&mut self, msg: &AppMsg) -> Result<()> {
        match msg {
            AppMsg::QuerySubmitted => {
                if !self.model.query_text.trim().is_empty() {
                    if let Some(subscription_id) = self.model.get_subscription_id() {
                        let query = self.model.query_text.clone();
                        let subscription_id = subscription_id.to_string();
                        let executor = self.query_executor.clone();
                        tokio::spawn(async move {
                            let _ = executor.execute_query(query, subscription_id).await;
                        });
                    } else {
                        // Not connected - send error message
                        let _ = self.sender.send(AppMsg::StatusMessage(
                            crate::message::StatusSeverity::Error,
                            "Cannot execute query: Not connected to server".to_string()
                        ));
                    }
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

    /// Sync UI components with model state with comprehensive logging
    #[instrument(level = "debug", skip(self), fields(
        focused_component = ?self.model.focused_component,
        query_length = self.model.query_text.len()
    ))]
    async fn sync_ui_with_model(&mut self) -> Result<()> {
        // Update query input text
        let query_sync_result = self.ui_app.attr(
            &ComponentId::QueryInput,
            tuirealm::Attribute::Text,
            tuirealm::AttrValue::String(self.model.query_text.clone()),
        );

        let sync_success = query_sync_result.is_ok();
        let error_msg = query_sync_result.err().map(|e| e.to_string());
        EventLogger::log_component_sync(
            &ComponentId::QueryInput,
            "text",
            sync_success,
            error_msg.as_deref(),
        );

        // Get previous focus
        let previous_focus = self.ui_app.focus().cloned().unwrap_or(ComponentId::Timeline);

        // Blur previous component if different
        if previous_focus != self.model.focused_component {
            let _ = self.ui_app.attr(
                &previous_focus,
                tuirealm::Attribute::Focus,
                tuirealm::AttrValue::Flag(false),
            );
        }

        // Focus new component
        let _ = self.ui_app.attr(
            &self.model.focused_component,
            tuirealm::Attribute::Focus,
            tuirealm::AttrValue::Flag(true),
        );

        // Set as active for routing
        match self.ui_app.active(&self.model.focused_component) {
            Ok(_) => {
                EventLogger::log_focus_change(&previous_focus, &self.model.focused_component, true);
            }
            Err(e) => {
                EventLogger::log_focus_change(&previous_focus, &self.model.focused_component, false);
                warn!(
                    from = ?previous_focus,
                    to = ?self.model.focused_component,
                    error = %e,
                    "Failed to set focus to component"
                );
            }
        }

        // Update StatusLine component with connection state
        let connection_state = if self.model.is_connected() {
            crate::components::realm::status_line::ConnectionState::Connected
        } else {
            crate::components::realm::status_line::ConnectionState::Disconnected
        };
        
        let _ = self.ui_app.attr(
            &ComponentId::StatusLine,
            tuirealm::Attribute::Custom("connection_state"),
            tuirealm::AttrValue::Payload(PropPayload::One(PropValue::Str(
                format!("{:?}", connection_state)
            ))),
        );

        // Update StatusLine component with status message
        let status_text = match &self.model.status_message {
            Some(msg) => msg.message.clone(),
            None => String::new(),
        };
        let _ = self.ui_app.attr(
            &ComponentId::StatusLine,
            tuirealm::Attribute::Custom("status_message"),
            tuirealm::AttrValue::Payload(PropPayload::One(PropValue::Str(status_text))),
        );

        Ok(())
    }

    /// Render the application with performance monitoring
    #[instrument(level = "trace", skip(self), fields(
        needs_render = self.needs_render,
    ))]
    fn render(&mut self) {
        let start_time = Instant::now();
        let model = &self.model;

        let result = self.terminal.draw(|frame| {
            render_app(model, &mut self.ui_app, frame);
        });

        let render_time = start_time.elapsed();

        EventLogger::log_render_decision(
            self.needs_render,
            Some(render_time.as_millis()),
        );

        // Warn on slow rendering
        if render_time.as_millis() > 16 {  // Target 60 FPS = ~16ms per frame
            warn!(
                render_time_ms = render_time.as_millis(),
                "Slow rendering detected (>16ms)"
            );
        }

        if let Err(e) = result {
            EventLogger::log_error(
                "render",
                &anyhow::anyhow!("Render failed: {}", e),
                None,
                true, // Rendering errors are usually recoverable
            );
        }
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


