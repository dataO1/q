//! HITL (Human-in-the-Loop) Queue Widget
//! 
//! Displays pending HITL approval requests in a chronological list

use crate::client::HitlApprovalRequest;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Modifier},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::collections::VecDeque;

/// HITL Queue component state
pub struct HitlQueueComponent {
    /// Pending HITL requests in chronological order
    requests: VecDeque<HitlApprovalRequest>,
    
    /// List state for selection handling
    list_state: ListState,
    
    /// Selected request index for keyboard navigation
    selected_index: usize,
}

/// Messages for HITL queue updates
#[derive(Debug, Clone)]
pub enum HitlQueueMessage {
    /// New HITL request received
    NewRequest(HitlApprovalRequest),
    /// Request was processed and should be removed
    RequestProcessed(String), // request_id
    /// Navigate up in list
    SelectPrevious,
    /// Navigate down in list  
    SelectNext,
    /// Clear all processed requests
    Clear,
}

impl HitlQueueComponent {
    /// Create new HITL queue component
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(None);
        
        Self {
            requests: VecDeque::new(),
            list_state,
            selected_index: 0,
        }
    }
    
    /// Update component state with a message
    pub fn update(&mut self, message: HitlQueueMessage) {
        match message {
            HitlQueueMessage::NewRequest(request) => {
                // Add to end of queue (chronological order)
                self.requests.push_back(request);
                
                // Auto-select first item if this is the first request
                if self.requests.len() == 1 {
                    self.selected_index = 0;
                    self.list_state.select(Some(0));
                }
            }
            
            HitlQueueMessage::RequestProcessed(request_id) => {
                // Remove processed request
                self.requests.retain(|req| req.request_id != request_id);
                
                // Adjust selection if needed
                if self.selected_index >= self.requests.len() && !self.requests.is_empty() {
                    self.selected_index = self.requests.len() - 1;
                }
                
                // Update list state
                if self.requests.is_empty() {
                    self.list_state.select(None);
                } else {
                    self.list_state.select(Some(self.selected_index));
                }
            }
            
            HitlQueueMessage::SelectPrevious => {
                if !self.requests.is_empty() && self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.list_state.select(Some(self.selected_index));
                }
            }
            
            HitlQueueMessage::SelectNext => {
                if !self.requests.is_empty() && self.selected_index < self.requests.len() - 1 {
                    self.selected_index += 1;
                    self.list_state.select(Some(self.selected_index));
                }
            }
            
            HitlQueueMessage::Clear => {
                self.requests.clear();
                self.selected_index = 0;
                self.list_state.select(None);
            }
        }
    }
    
    /// Get currently selected request
    pub fn get_selected_request(&self) -> Option<&HitlApprovalRequest> {
        self.requests.get(self.selected_index)
    }
    
    /// Get number of pending requests
    pub fn len(&self) -> usize {
        self.requests.len()
    }
    
    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
    
    /// Render the HITL queue widget
    pub fn render(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        let title = format!(" HITL Queue ({}) ", self.requests.len());
        
        let border_style = if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };
        
        if self.requests.is_empty() {
            // Show empty state
            let empty_msg = vec![
                Line::from(vec![
                    Span::styled("No pending HITL requests", Style::default().fg(Color::Gray))
                ])
            ];
            
            let paragraph = Paragraph::new(empty_msg)
                .block(Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(border_style))
                .wrap(Wrap { trim: false });
            
            f.render_widget(paragraph, area);
            return;
        }
        
        // Convert requests to ListItems
        let items: Vec<ListItem> = self.requests
            .iter()
            .enumerate()
            .map(|(i, request)| {
                let is_selected = i == self.selected_index;
                
                // Format: "Agent: proposed_action (risk_level)"
                let content = format!("{}: {} ({})", 
                    request.agent_type, 
                    request.proposed_action, 
                    request.risk_level
                );
                
                let style = if is_selected && focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else if matches!(request.risk_level.as_str(), "HIGH") {
                    Style::default().fg(Color::Red)
                } else if matches!(request.risk_level.as_str(), "MEDIUM") {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                };
                
                ListItem::new(Line::from(vec![
                    Span::styled(content, style)
                ]))
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style))
            .highlight_style(Style::default().bg(Color::DarkGray));
        
        f.render_stateful_widget(list, area, &mut self.list_state);
    }
}

impl Default for HitlQueueComponent {
    fn default() -> Self {
        Self::new()
    }
}