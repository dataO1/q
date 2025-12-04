//! Application layer - Core Elm architecture implementation
//!
//! This module implements the Model-Update-View pattern from Elm architecture.

pub mod state;
pub mod update;
pub mod view;

use chrono::Utc;
pub use state::AppModel;
pub use view::render;

use std::{io, sync::Arc, time::{Duration, Instant}, pin::Pin};
use anyhow::{Context, Result};
use crossterm::{execute, terminal::{disable_raw_mode, LeaveAlternateScreen}};
use tokio::{sync::mpsc, time::interval, signal};
use tracing::{debug, info, trace, warn, error, instrument};
use futures::{FutureExt, Future};
use tuirealm::{
    application::PollStrategy, listener::{AsyncPort, ListenerResult, PollAsync}, props::{PropPayload, PropValue}, terminal::{CrosstermTerminalAdapter, TerminalBridge}, Application as TuiApplication, AttrValue, Attribute, Event, EventListenerCfg, Sub, SubClause, SubEventClause
};

use crate::{
    client::{AcpClient, EventSource, EventType, StatusEvent}, components::{realm::{
        event_tree::EventTreeRealmComponent, help::HelpRealmComponent, root::RootRealmComponent, status_line::ConnectionState, QueryInputRealmComponent, StatusLineRealmComponent
    }, HitlReviewRealmComponent}, config::Config, log_state_change, message::{APIEvent, ComponentId,  StatusSeverity, UserEvent}, services::{ApiService, QueryExecutor, WebSocketManager}, time_operation, utils::{generate_client_id, EventLogger}
};

/// Async channel wrapper that implements PollAsync for AppMsg
struct APIEventChannel {
    receiver: mpsc::UnboundedReceiver<APIEvent>,
}

impl APIEventChannel {
    fn new(receiver: mpsc::UnboundedReceiver<APIEvent>) -> Self {
        Self { receiver }
    }
}

#[tuirealm::async_trait]
impl PollAsync<APIEvent> for APIEventChannel {
    async fn poll(&mut self) -> ListenerResult<Option<Event<APIEvent>>> {
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
    app: TuiApplication<ComponentId, UserEvent, APIEvent>,
    /// Terminal interface
    terminal: TerminalBridge<CrosstermTerminalAdapter>,
    /// Application model (state)
    model: AppModel,
    /// Message sender for async operations
    sender: mpsc::UnboundedSender<APIEvent>,
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
        let mut app = TuiApplication::init(
            EventListenerCfg::default()
                .async_crossterm_input_listener(Duration::from_millis(0), 3) // Responsive keyboard input
                .add_async_port(
                    Box::new(APIEventChannel::new(receiver)),
                    Duration::from_millis(10), // Poll interval
                    100 // Max poll count
                ) // Add our async message channel
                .with_handle(tokio::runtime::Handle::current()) // Enable async runtime
        );

        // Mount all components
        app.mount(
            ComponentId::Root,
            Box::new(RootRealmComponent::new()),
            vec![
                Sub::new(
                    SubEventClause::Any,
                    SubClause::Always,      // Always receive them
                ),
            ],
        )
        .context("Failed to mount QueryInput component")?;
        app.mount(
            ComponentId::Timeline,
            Box::new(EventTreeRealmComponent::new()),
            vec![
                Sub::new(
                    SubEventClause::Any,
                    SubClause::Always,      // Always receive them
                ),
            ],
        )?;

        app.mount(ComponentId::QueryInput, Box::new(QueryInputRealmComponent::new()), vec![])
            .context("Failed to mount QueryInput component")?;

        app.mount(ComponentId::StatusLine, Box::new(StatusLineRealmComponent::new()), vec![])
            .context("Failed to mount StatusLine component")?;

        app.mount(ComponentId::HitlReview, Box::new(HitlReviewRealmComponent::new()),
            vec![
                Sub::new(
                    SubEventClause::Any,
                    SubClause::Always,      // Always receive them
                ),
            ],
        )
            .context("Failed to mount HitlReview component")?;

        app.mount(ComponentId::Help, Box::new(HelpRealmComponent::new()), vec![])
            .context("Failed to mount HelpRealmComponent component")?;


        // Set initial focus
        app.active(&ComponentId::QueryInput)
            .context("Failed to set initial focus")?;

        // Initialize services
        let acp_client = Arc::new(AcpClient::new(&config.server_url)?);
        let api_service = ApiService::new(acp_client.clone());
        let query_executor = QueryExecutor::new(api_service.clone(), sender.clone());
        let websocket_manager = WebSocketManager::new(config.server_url.clone(), sender.clone())?;

        // Animation timer - configurable rate for smooth animations when needed
        let animation_timer = interval(Duration::from_millis(config.ui.animation_interval_ms));

        info!("Application initialized successfully");

        Ok(Self {
            app,
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
        let mut tick_interval = tokio::time::interval(Duration::from_millis(50)); // 10 FPS
        tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Force initial render to show the UI immediately
        self.render();

        'main_loop: loop {
            tokio::select! {
                // Fast terminal event polling
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    match self.app.tick(PollStrategy::Once) {
                        Ok(events) if !events.is_empty() => {
                            for event in events {
                                if self.handle_event(event).await? {
                                    break 'main_loop;
                                }
                            }
                            self.render();
                        }
                        Ok(_) => {}
                        Err(e) => error!("Tick error: {}", e),
                    }
                }

                // Animation tick (also drains WebSocket via component)
                _ = tick_interval.tick() => {
                    if self.handle_event(UserEvent::Tick).await? {
                        break 'main_loop;
                    }
                    self.render();
                }

                _ = ctrl_c.as_mut() => {
                    info!("Received Ctrl+C");
                    break 'main_loop;
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

    /// Handle a message using the Elm update pattern
    #[instrument(level = "debug", skip(self), fields(
        message_type = %format!("{:?}", msg).split('(').next().unwrap_or("Unknown"),
        needs_render = self.needs_render
    ))]
    async fn handle_event(&mut self, msg: UserEvent) -> Result<bool> {
        let start_time = Instant::now();
        let model_before = self.model.clone();

        // Check for quit message first
        if matches!(msg, UserEvent::Quit) {
            info!("Quit message received, initiating shutdown");
            EventLogger::log_lifecycle_event("shutdown_requested", None);
            return Ok(true);
        }

        debug!(?msg, "Processing message");

        // Update model with Elm update function
        time_operation!("model_update", {
            self.update(msg.clone())
        }).await?;

        // Log state changes
        log_state_change!(&model_before, &self.model, &format!("{:?}", msg));

        // Mark that we need to render since the model has changed
        self.needs_render = true;


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

    /// Sync UI components with model state with comprehensive logging
    #[instrument(level = "debug", skip(self), fields(
        focused_component = ?self.model.focused_component,
    ))]
    async fn sync_ui_with_model(&mut self) -> Result<()> {
        // Get previous focus
        let previous_focus = self.app.focus().cloned().unwrap_or(ComponentId::Timeline);

        // Blur previous component if different
        if previous_focus != self.model.focused_component {
            let _ = self.app.attr(
                &previous_focus,
                tuirealm::Attribute::Focus,
                tuirealm::AttrValue::Flag(false),
            );
        }

        // Focus new component
        let _ = self.app.attr(
            &self.model.focused_component,
            tuirealm::Attribute::Focus,
            tuirealm::AttrValue::Flag(true),
        );

        // Set as active for routing
        match self.app.active(&self.model.focused_component) {
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
        let connection_state = &self.model.connection_state;
        let _ = self.app.attr(
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
        let _ = self.app.attr(
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
            render(model, &mut self.app, frame);
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


    /// Update function - handles all application messages and updates the model
    /// This is the core Update function from Elm architecture
    #[instrument(level = "debug", skip(self), fields(
        msg_type = %format!("{:?}", msg).split('(').next().unwrap_or("Unknown"),
        focused_component = ?self.model.focused_component,
    ))]
    pub async fn update(&mut self, msg: UserEvent) -> Result<()> {
        let model = &mut self.model;

        match msg {
            // ============== System Events ==============
            UserEvent::Quit => {
                info!("Application quit requested");
                // No model changes needed, handled by main loop
            }

            UserEvent::Tick => {
                let _ = self.app.attr(
                    &ComponentId::Timeline,
                    Attribute::Custom("tick"),
                    AttrValue::Flag(true),
                );
            }

            // ============== Connection Events ==============
            UserEvent::StartConnection => {
                info!("Starting connection to ACP server");
                model.set_connection_state(crate::components::realm::status_line::ConnectionState::Connecting);
                model.set_status_message(StatusSeverity::Info, "Connecting to ACP server...".to_string());
            }

            UserEvent::WebSocketConnected(subscription_id) => {
                info!(
                    subscription_id = %subscription_id,
                    previous_connection_state = ?model.connection_state,
                    "WebSocket connected with subscription"
                );
                model.connection_state = ConnectionState::Connected(subscription_id.clone());
                model.set_status_message(StatusSeverity::Info,
                    format!("Connected (subscription: {})", subscription_id));
                info!(
                    subscription_id = %subscription_id,
                    new_connection_state = ?model.connection_state,
                    "Connection state updated"
                );
            }

            UserEvent::SubscriptionResumed(subscription_id) => {
                info!(subscription_id = %subscription_id, "Subscription resumed");
                model.connection_state = ConnectionState::Connected(subscription_id.clone());
                model.set_status_message(StatusSeverity::Info,
                    format!("Reconnected (subscription: {})", subscription_id));
            }

            UserEvent::WebSocketDisconnected => {
                warn!("WebSocket disconnected");
                model.connection_state = ConnectionState::Disconnected;
                model.set_connection_state(crate::components::realm::status_line::ConnectionState::Disconnected);
                model.set_status_message(StatusSeverity::Warning, "WebSocket disconnected".to_string());
            }

            UserEvent::ConnectionFailed(error) => {
                warn!(error = %error, "Connection failed");
                model.connection_state = ConnectionState::Failed{error: error.clone()};
                model.set_connection_state(crate::components::realm::status_line::ConnectionState::Failed {
                    error: error.clone()
                });
                model.set_status_message(StatusSeverity::Error,
                    format!("Connection failed: {}", error));
            }


            UserEvent::QuerySubmitted(query) => {
                debug!(
                    query_text = %query,
                    query_len = query.len(),
                    connection_state = ?model.connection_state,
                    "Processing query submission"
                );

                if !query.trim().is_empty() {
                    info!(query = %query, query_len = query.len(), "Query submitted");
                    model.last_execution_time = Some(Utc::now());


                    // Trigger query execution effect
                    debug!(query = %query, "Creating QueryExecutionStarted effect");
                    if let ConnectionState::Connected(sub_id) = &self.model.connection_state{
                        self.query_executor.execute_query(query,sub_id.clone()).await?;
                    }
                } else {
                    warn!("Query submission attempted with empty query text");
                }
            }

            UserEvent::QueryExecutionStarted(query) => {
                info!(
                    query = %query,
                    connection_state = ?model.connection_state,
                    "Query execution started"
                );
                model.set_status_message(StatusSeverity::Info,
                    format!("Executing query: {}", query));
            }

            UserEvent::QueryExecutionCompleted(result) => {
                info!(result = %result, "Query execution completed");
                model.set_status_message(StatusSeverity::Info,
                    format!("Query completed: {}", result));
            }

            UserEvent::QueryExecutionFailed(error) => {
                warn!(error = %error, "Query execution failed");
                model.set_status_message(StatusSeverity::Error,
                    format!("Query failed: {}", error));
            }

            // ============== UI Navigation Events ==============
            UserEvent::FocusNext => {
                debug!(from = ?model.focused_component, "Focus next component");
                model.focus_next_component();
                debug!(to = ?model.focused_component, "Focused next component");
            }

            UserEvent::FocusPrevious => {
                debug!(from = ?model.focused_component, "Focus previous component");
                model.focus_previous_component();
                debug!(to = ?model.focused_component, "Focused previous component");
            }

            UserEvent::FocusComponent(component_id) => {
                debug!(from = ?model.focused_component, to = ?component_id, "Focus specific component");
                model.focused_component = component_id;
            }

            UserEvent::HelpToggle => {
                debug!(show_help = !model.show_help, "Toggle help");
                model.toggle_help();
            }

            // ============== Error Events ==============
            UserEvent::ErrorOccurred(error) => {
                warn!(error = %error, "Error occurred");
                model.set_status_message(StatusSeverity::Error, error);
            }

            UserEvent::StatusMessage(severity, message) => {
                debug!(severity = ?severity, message = %message, "Status message");
                model.set_status_message(severity, message);
            }

            UserEvent::HitlDecisionPending =>{
                model.hitl_popup_open();
            }

            UserEvent::HitlDecisionSubmit{id, approved, modified_content, reasoning } =>{
                if let ConnectionState::Connected(conversation_id) = &model.connection_state{
                    let event = EventType::HitlDecision{id: id.clone(), approved, modified_content, reasoning};
                    let event = StatusEvent{conversation_id: conversation_id.clone(), timestamp: Utc::now(), source: EventSource::Hitl{request_id:id}, event};
                    if let Ok(()) = self.websocket_manager.submit_hitl_decision(event).await{
                        model.hitl_popup_close();
                    };
                }
            }
        }

        Ok(())
    }
}


