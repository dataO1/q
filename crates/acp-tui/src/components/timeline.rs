//! Timeline component with React/Elm-style message-driven state transitions
//!
//! This component maintains tree state and responds to incoming WebSocket messages
//! with pure state transition functions, following React/Elm architecture patterns.

use crate::models::{StatusEvent, EventSource, EventType};
use crate::models::tree::{TimelineTree, TreeNode, NodeStatus};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::collections::HashMap;

/// Timeline widget component following React/Elm patterns
pub struct TimelineComponent {
    /// Current tree state
    tree: TimelineTree,
    
    /// Scroll offset for viewing large trees
    scroll_offset: usize,
    
    /// Active node tracking
    active_nodes: HashMap<String, ActiveNodeState>,
    
    /// Animation tick counter
    animation_tick: usize,
}

/// State of an active node for animation and tracking
#[derive(Debug, Clone)]
struct ActiveNodeState {
    /// When this node became active
    started_at: std::time::Instant,
    /// Last known activity description
    current_activity: Option<String>,
    /// Tool chain (for nested tool calls)
    tool_chain: Vec<String>,
}

/// Messages that trigger state transitions
#[derive(Debug, Clone)]
pub enum TimelineMessage {
    /// WebSocket event received
    StatusEvent(StatusEvent),
    /// Animation tick
    AnimationTick,
    /// Scroll up
    ScrollUp,
    /// Scroll down
    ScrollDown,
    /// Reset timeline
    Reset,
}

impl TimelineComponent {
    /// Create new timeline component
    pub fn new() -> Self {
        Self {
            tree: TimelineTree::new(),
            scroll_offset: 0,
            active_nodes: HashMap::new(),
            animation_tick: 0,
        }
    }
    
    /// Update component state with a message (pure function)
    pub fn update(&mut self, message: TimelineMessage) {
        match message {
            TimelineMessage::StatusEvent(event) => {
                self.handle_status_event(event);
            }
            TimelineMessage::AnimationTick => {
                self.animation_tick += 1;
                self.tree.advance_animations();
            }
            TimelineMessage::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            TimelineMessage::ScrollDown => {
                self.scroll_offset += 1;
            }
            TimelineMessage::Reset => {
                self.tree.clear();
                self.active_nodes.clear();
                self.scroll_offset = 0;
                self.animation_tick = 0;
            }
        }
    }
    
    /// Handle incoming status events with state transitions
    fn handle_status_event(&mut self, event: StatusEvent) {
        let node_id = self.extract_node_id(&event.source);
        let display_name = self.extract_display_name(&event);
        
        match &event.event {
            // Execution started - create root node
            EventType::ExecutionStarted { query } => {
                let root = self.tree.add_root(node_id.clone(), query.clone());
                root.start();
                self.track_active_node(node_id, None);
            }
            
            // Agent started - create or update agent node
            EventType::AgentStarted { .. } => {
                self.ensure_agent_node(&event.source, &display_name);
                if let Some(node) = self.tree.find_node_mut(&node_id) {
                    node.start();
                }
                self.track_active_node(node_id, None);
            }
            
            // Agent thinking - update activity
            EventType::AgentThinking { thought } => {
                self.update_agent_activity(&node_id, thought.clone());
            }
            
            // Tool started - create tool node under agent
            EventType::ToolStarted { args } => {
                if let EventSource::Tool { agent_id, tool_name } = &event.source {
                    self.ensure_agent_node(&EventSource::Agent { 
                        agent_id: agent_id.clone(), 
                        agent_type: "unknown".to_string() 
                    }, agent_id);
                    
                    let tool_display = format!("{} {}", tool_name, 
                        args.as_str().unwrap_or(""));
                    
                    self.tree.add_child(agent_id.clone(), node_id.clone(), tool_display);
                    if let Some(node) = self.tree.find_node_mut(&node_id) {
                        node.start();
                    }
                    
                    // Add to tool chain
                    if let Some(active) = self.active_nodes.get_mut(agent_id) {
                        active.tool_chain.push(tool_name.clone());
                    }
                }
            }
            
            // Tool completed
            EventType::ToolCompleted { result: _ } => {
                if let Some(node) = self.tree.find_node_mut(&node_id) {
                    node.complete();
                }
                
                // Remove from tool chain
                if let EventSource::Tool { agent_id, tool_name } = &event.source {
                    if let Some(active) = self.active_nodes.get_mut(agent_id) {
                        active.tool_chain.retain(|t| t != tool_name);
                    }
                }
            }
            
            // Tool failed
            EventType::ToolFailed { error } => {
                if let Some(node) = self.tree.find_node_mut(&node_id) {
                    node.fail(Some(error.clone()));
                }
                
                // Remove from tool chain and mark agent as warning
                if let EventSource::Tool { agent_id, .. } = &event.source {
                    if let Some(agent_node) = self.tree.find_node_mut(agent_id) {
                        agent_node.warn(format!("Tool failed: {}", error));
                    }
                }
            }
            
            // Agent completed
            EventType::AgentCompleted { result: _ } => {
                if let Some(node) = self.tree.find_node_mut(&node_id) {
                    node.complete();
                }
                self.active_nodes.remove(&node_id);
            }
            
            // Agent failed
            EventType::AgentFailed { error } => {
                if let Some(node) = self.tree.find_node_mut(&node_id) {
                    node.fail(Some(error.clone()));
                }
                self.active_nodes.remove(&node_id);
            }
            
            // Execution completed
            EventType::ExecutionCompleted { result: _ } => {
                if let Some(node) = self.tree.find_node_mut(&node_id) {
                    node.complete();
                }
                self.active_nodes.clear();
            }
            
            // Execution failed
            EventType::ExecutionFailed { error } => {
                if let Some(node) = self.tree.find_node_mut(&node_id) {
                    node.fail(Some(error.clone()));
                }
                self.active_nodes.clear();
            }
            
            // Workflow steps
            EventType::WorkflowStepStarted { step_name } => {
                let step_node_id = format!("{}::{}", node_id, step_name);
                self.tree.add_child(node_id, step_node_id.clone(), step_name.clone());
                if let Some(node) = self.tree.find_node_mut(&step_node_id) {
                    node.start();
                }
            }
            
            EventType::WorkflowStepCompleted { step_name } => {
                let step_node_id = format!("{}::{}", node_id, step_name);
                if let Some(node) = self.tree.find_node_mut(&step_node_id) {
                    node.complete();
                }
            }
            
            // HITL events
            EventType::HitlRequested { task_description, risk_level } => {
                let hitl_display = format!("HITL: {} ({})", task_description, risk_level);
                self.tree.add_child(node_id, format!("hitl::{}", event.execution_id), hitl_display);
            }
            
            EventType::HitlCompleted { approved, reason } => {
                let hitl_id = format!("hitl::{}", event.execution_id);
                if let Some(node) = self.tree.find_node_mut(&hitl_id) {
                    if *approved {
                        node.complete();
                    } else {
                        node.fail(reason.clone());
                    }
                }
            }
        }
    }
    
    /// Extract node ID from event source
    fn extract_node_id(&self, source: &EventSource) -> String {
        match source {
            EventSource::Orchestrator => "orchestrator".to_string(),
            EventSource::Agent { agent_id, .. } => agent_id.clone(),
            EventSource::Tool { tool_name, agent_id } => {
                format!("{}::{}", agent_id, tool_name)
            }
            EventSource::Workflow { node_id, .. } => node_id.clone(),
            EventSource::Hitl { request_id } => format!("hitl::{}", request_id),
        }
    }
    
    /// Extract display name from event
    fn extract_display_name(&self, event: &StatusEvent) -> String {
        match &event.source {
            EventSource::Orchestrator => "Orchestrator".to_string(),
            EventSource::Agent { agent_type, .. } => {
                // Extract readable agent type
                let clean_type = agent_type
                    .replace("_", " ")
                    .replace("-", " ")
                    .split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                
                // Add emoji based on agent type
                let emoji = self.get_agent_emoji(agent_type);
                format!("{} {}", emoji, clean_type)
            }
            EventSource::Tool { tool_name, .. } => {
                let emoji = self.get_tool_emoji(tool_name);
                format!("{} {}", emoji, tool_name)
            }
            EventSource::Workflow { node_id, .. } => {
                format!("📋 {}", node_id)
            }
            EventSource::Hitl { .. } => "👤 Human Review".to_string(),
        }
    }
    
    /// Get emoji for agent type (dynamic discovery)
    fn get_agent_emoji(&self, agent_type: &str) -> &'static str {
        let lower_type = agent_type.to_lowercase();
        if lower_type.contains("plan") {
            "📋"
        } else if lower_type.contains("cod") || lower_type.contains("dev") {
            "💻"
        } else if lower_type.contains("review") || lower_type.contains("eval") {
            "🔍"
        } else if lower_type.contains("test") {
            "🧪"
        } else if lower_type.contains("write") || lower_type.contains("doc") {
            "📝"
        } else {
            "🤖" // Default robot emoji
        }
    }
    
    /// Get emoji for tool name (dynamic discovery)
    fn get_tool_emoji(&self, tool_name: &str) -> &'static str {
        let lower_tool = tool_name.to_lowercase();
        if lower_tool.contains("read") || lower_tool.contains("file") {
            "📄"
        } else if lower_tool.contains("write") || lower_tool.contains("edit") {
            "✏️"
        } else if lower_tool.contains("search") || lower_tool.contains("find") {
            "🔎"
        } else if lower_tool.contains("exec") || lower_tool.contains("run") {
            "⚡"
        } else if lower_tool.contains("git") {
            "🌳"
        } else if lower_tool.contains("build") || lower_tool.contains("compile") {
            "🔨"
        } else if lower_tool.contains("test") {
            "🧪"
        } else {
            "🔧" // Default tool emoji
        }
    }
    
    /// Ensure agent node exists
    fn ensure_agent_node(&mut self, source: &EventSource, display_name: &str) {
        if let EventSource::Agent { agent_id, .. } = source {
            if self.tree.find_node_mut(agent_id).is_none() {
                // Create as root if no parent found
                self.tree.add_root(agent_id.clone(), display_name.to_string());
            }
        }
    }
    
    /// Track active node state
    fn track_active_node(&mut self, node_id: String, activity: Option<String>) {
        self.active_nodes.insert(node_id, ActiveNodeState {
            started_at: std::time::Instant::now(),
            current_activity: activity,
            tool_chain: Vec::new(),
        });
    }
    
    /// Update agent activity
    fn update_agent_activity(&mut self, node_id: &str, activity: String) {
        if let Some(active) = self.active_nodes.get_mut(node_id) {
            active.current_activity = Some(activity.clone());
        }
        
        // Update node display with current activity
        if let Some(node) = self.tree.find_node_mut(node_id) {
            // Add activity as a temporary child or update description
            let activity_id = format!("{}::activity", node_id);
            
            // Remove previous activity node
            node.children.retain(|child| !child.id.starts_with(&format!("{}::activity", node_id)));
            
            // Add new activity node
            let mut activity_node = TreeNode::new(activity_id, format!("💭 {}", activity), node.depth + 1);
            activity_node.status = NodeStatus::Running;
            node.add_child(activity_node);
        }
    }
    
    /// Render the timeline component
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let lines = self.tree.render_lines();
        
        // Apply scroll offset
        let visible_lines: Vec<Line> = lines
            .iter()
            .skip(self.scroll_offset)
            .take(area.height as usize - 2) // Account for borders
            .map(|line| {
                Line::from(self.style_line(line))
            })
            .collect();
        
        let stats = self.tree.get_stats();
        let title = format!(" Timeline (⟳{} ✔{} ✗{} ⚠{}) ", 
            stats.running, stats.completed, stats.failed, stats.warnings);
        
        let paragraph = Paragraph::new(visible_lines)
            .block(Block::default()
                .title(title)
                .borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        
        f.render_widget(paragraph, area);
    }
    
    /// Apply styling to a line based on status indicators
    fn style_line<'a>(&self, line: &'a str) -> Vec<Span<'a>> {
        let mut spans = Vec::new();
        
        // Basic line styling based on content
        let style = if line.contains("✗") {
            Style::default().fg(Color::Red)
        } else if line.contains("⚠") {
            Style::default().fg(Color::Yellow)
        } else if line.contains("✔") {
            Style::default().fg(Color::Green)
        } else if line.contains("⠋") || line.contains("⠙") || line.contains("⠹") || 
                  line.contains("⠸") || line.contains("⠼") || line.contains("⠴") ||
                  line.contains("⠦") || line.contains("⠧") || line.contains("⠇") || line.contains("⠏") {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        
        spans.push(Span::styled(line, style));
        spans
    }
    
    /// Get current scroll info
    pub fn scroll_info(&self) -> (usize, usize) {
        let total_lines = self.tree.render_lines().len();
        (self.scroll_offset, total_lines)
    }
}

impl Default for TimelineComponent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{StatusEvent, EventSource, EventType};
    use chrono::Utc;
    
    #[test]
    fn test_timeline_message_handling() {
        let mut timeline = TimelineComponent::new();
        
        // Test execution started
        let event = StatusEvent {
            execution_id: "test123".to_string(),
            timestamp: Utc::now(),
            source: EventSource::Orchestrator,
            event: EventType::ExecutionStarted { 
                query: "Test query".to_string() 
            },
        };
        
        timeline.update(TimelineMessage::StatusEvent(event));
        
        assert_eq!(timeline.tree.roots.len(), 1);
        assert_eq!(timeline.tree.roots[0].display_name, "Test query");
    }
    
    #[test]
    fn test_agent_emoji_selection() {
        let timeline = TimelineComponent::new();
        
        assert_eq!(timeline.get_agent_emoji("planner"), "📋");
        assert_eq!(timeline.get_agent_emoji("coder"), "💻");
        assert_eq!(timeline.get_agent_emoji("reviewer"), "🔍");
        assert_eq!(timeline.get_agent_emoji("unknown_agent"), "🤖");
    }
    
    #[test]
    fn test_tool_emoji_selection() {
        let timeline = TimelineComponent::new();
        
        assert_eq!(timeline.get_tool_emoji("read_file"), "📄");
        assert_eq!(timeline.get_tool_emoji("exec_command"), "⚡");
        assert_eq!(timeline.get_tool_emoji("search_code"), "🔎");
        assert_eq!(timeline.get_tool_emoji("unknown_tool"), "🔧");
    }
}