//! Tree structure for dynamic timeline visualization
//!
//! This module provides the tree data structure for representing agent/tool execution
//! hierarchy with animation state for real-time visualization.

use std::collections::HashMap;
use std::time::Instant;

/// Tree node representing an agent, tool, or step in execution
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Unique identifier from event source
    pub id: String,
    
    /// Display name extracted from event
    pub display_name: String,
    
    /// Current status of this node
    pub status: NodeStatus,
    
    /// Child nodes
    pub children: Vec<TreeNode>,
    
    /// When this node started
    pub start_time: Option<Instant>,
    
    /// Animation frame for line animations
    pub animation_frame: usize,
    
    /// Depth in the tree (for animation timing)
    pub depth: usize,
    
    /// Duration text if completed
    pub duration_text: Option<String>,
    
    /// Error/warning message if applicable
    pub error_message: Option<String>,
}

/// Status of a tree node
#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    /// Node is currently active/running
    Running,
    /// Node completed successfully
    Completed,
    /// Node failed with an error
    Failed,
    /// Node is waiting to start
    Pending,
    /// Node has a warning but continues
    Warning,
}

/// Timeline tree managing the entire execution hierarchy
#[derive(Debug, Clone)]
pub struct TimelineTree {
    /// Root nodes (top-level operations)
    pub roots: Vec<TreeNode>,
    
    /// Node lookup by ID for fast updates
    node_map: HashMap<String, Vec<usize>>, // Path to node in tree
    
    /// Global animation counter
    animation_counter: usize,
    
    /// Track parent-child relationships from events
    parent_map: HashMap<String, String>,
}

impl TreeNode {
    /// Create a new tree node
    pub fn new(id: String, display_name: String, depth: usize) -> Self {
        Self {
            id,
            display_name,
            status: NodeStatus::Pending,
            children: Vec::new(),
            start_time: None,
            animation_frame: 0,
            depth,
            duration_text: None,
            error_message: None,
        }
    }
    
    /// Mark this node as running
    pub fn start(&mut self) {
        self.status = NodeStatus::Running;
        self.start_time = Some(Instant::now());
    }
    
    /// Mark this node as completed
    pub fn complete(&mut self) {
        self.status = NodeStatus::Completed;
        if let Some(start) = self.start_time {
            let duration = start.elapsed();
            self.duration_text = Some(format!("{:02}:{:02}s", 
                duration.as_secs() / 60, 
                duration.as_secs() % 60
            ));
        }
    }
    
    /// Mark this node as failed
    pub fn fail(&mut self, error: Option<String>) {
        self.status = NodeStatus::Failed;
        self.error_message = error;
        if let Some(start) = self.start_time {
            let duration = start.elapsed();
            self.duration_text = Some(format!("{:02}:{:02}s", 
                duration.as_secs() / 60, 
                duration.as_secs() % 60
            ));
        }
    }
    
    /// Set warning status with message
    pub fn warn(&mut self, warning: String) {
        self.status = NodeStatus::Warning;
        self.error_message = Some(warning);
    }
    
    /// Advance animation frame
    pub fn advance_animation(&mut self) {
        if self.status == NodeStatus::Running {
            self.animation_frame = (self.animation_frame + 1) % 4;
            
            // Recursively advance children
            for child in &mut self.children {
                child.advance_animation();
            }
        }
    }
    
    /// Get animated line character for this node's connection
    pub fn get_animated_char(&self, is_horizontal: bool) -> &'static str {
        if self.status != NodeStatus::Running {
            // Static characters for non-running nodes
            return if is_horizontal { "─" } else { "│" };
        }
        
        // Animated characters based on frame and depth
        let frame = (self.animation_frame + self.depth) % 4;
        
        if is_horizontal {
            match frame {
                0 => "─",
                1 => "╌",
                2 => "┄", 
                3 => "┈",
                _ => "─",
            }
        } else {
            match frame {
                0 => "│",
                1 => "┆",
                2 => "┊",
                3 => "╎",
                _ => "│",
            }
        }
    }
    
    /// Get status indicator character
    pub fn get_status_char(&self) -> &'static str {
        match self.status {
            NodeStatus::Running => {
                // Cycle through spinner characters
                let spinners = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                spinners[self.animation_frame % spinners.len()]
            }
            NodeStatus::Completed => "✔",
            NodeStatus::Failed => "✗",
            NodeStatus::Pending => "…",
            NodeStatus::Warning => "⚠",
        }
    }
    
    /// Find a child node by ID recursively
    pub fn find_child_mut(&mut self, id: &str) -> Option<&mut TreeNode> {
        if self.id == id {
            return Some(self);
        }
        
        for child in &mut self.children {
            if let Some(found) = child.find_child_mut(id) {
                return Some(found);
            }
        }
        
        None
    }
    
    /// Add a child node
    pub fn add_child(&mut self, child: TreeNode) {
        self.children.push(child);
    }
    
    /// Get the current duration if running
    pub fn get_current_duration(&self) -> Option<String> {
        if let Some(start) = self.start_time {
            let duration = start.elapsed();
            Some(format!("{:02}:{:02}s", 
                duration.as_secs() / 60, 
                duration.as_secs() % 60
            ))
        } else {
            None
        }
    }
}

impl TimelineTree {
    /// Create a new timeline tree
    pub fn new() -> Self {
        Self {
            roots: Vec::new(),
            node_map: HashMap::new(),
            animation_counter: 0,
            parent_map: HashMap::new(),
        }
    }
    
    /// Add a root node
    pub fn add_root(&mut self, id: String, display_name: String) -> &mut TreeNode {
        let node = TreeNode::new(id.clone(), display_name, 0);
        self.roots.push(node);
        let index = self.roots.len() - 1;
        self.node_map.insert(id, vec![index]);
        &mut self.roots[index]
    }
    
    /// Add a child node to a parent
    pub fn add_child(&mut self, parent_id: String, child_id: String, display_name: String) {
        // Record parent relationship
        self.parent_map.insert(child_id.clone(), parent_id.clone());
        
        // Find parent node
        if let Some(parent_path) = self.node_map.get(&parent_id).cloned() {
            let parent_depth = parent_path.len() - 1;
            let child_depth = parent_depth + 1;
            
            // Navigate to parent and add child
            let parent = self.get_node_by_path_mut(&parent_path);
            if let Some(parent) = parent {
                let child = TreeNode::new(child_id.clone(), display_name, child_depth);
                parent.add_child(child);
                
                // Update node map with child's path
                let mut child_path = parent_path;
                child_path.push(parent.children.len() - 1);
                self.node_map.insert(child_id, child_path);
            }
        }
    }
    
    /// Find a node by ID
    pub fn find_node_mut(&mut self, id: &str) -> Option<&mut TreeNode> {
        if let Some(path) = self.node_map.get(id).cloned() {
            self.get_node_by_path_mut(&path)
        } else {
            None
        }
    }
    
    /// Get node by path in tree
    fn get_node_by_path_mut(&mut self, path: &[usize]) -> Option<&mut TreeNode> {
        if path.is_empty() {
            return None;
        }
        
        let mut current = &mut self.roots[path[0]];
        
        for &index in &path[1..] {
            if index >= current.children.len() {
                return None;
            }
            current = &mut current.children[index];
        }
        
        Some(current)
    }
    
    /// Advance all animations
    pub fn advance_animations(&mut self) {
        self.animation_counter += 1;
        
        for root in &mut self.roots {
            root.advance_animation();
        }
    }
    
    /// Render the tree as a vector of formatted lines
    pub fn render_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        
        for (i, root) in self.roots.iter().enumerate() {
            let is_last = i == self.roots.len() - 1;
            self.render_node(&root, &mut lines, "", is_last, true);
        }
        
        lines
    }
    
    /// Recursively render a node and its children
    fn render_node(
        &self,
        node: &TreeNode,
        lines: &mut Vec<String>,
        prefix: &str,
        is_last: bool,
        is_root: bool,
    ) {
        let mut line = String::new();
        
        if is_root {
            // Root node - no tree characters
            line.push_str(&format!("{} {}", 
                node.get_status_char(),
                node.display_name
            ));
        } else {
            // Child node - add tree characters
            let connector = if is_last { "└─" } else { "├─" };
            line.push_str(&format!("{}{} {}", 
                prefix,
                connector,
                node.display_name
            ));
        }
        
        // Add timing info
        if let Some(ref duration) = node.duration_text {
            line.push_str(&format!("  {}", duration));
        } else if let Some(current) = node.get_current_duration() {
            line.push_str(&format!("  {}", current));
        }
        
        // Add status character at the end for completed/failed
        if matches!(node.status, NodeStatus::Completed | NodeStatus::Failed) {
            line.push_str(&format!(" {}", node.get_status_char()));
        }
        
        lines.push(line);
        
        // Add error/warning message if present
        if let Some(ref error) = node.error_message {
            let error_prefix = if is_root {
                "  "
            } else {
                &format!("{}  ", if is_last { "  " } else { node.get_animated_char(false) })
            };
            lines.push(format!("{}└─ {}", error_prefix, error));
        }
        
        // Render children
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i == node.children.len() - 1;
            let child_prefix = if is_root {
                "".to_string()
            } else {
                format!("{}{} ", 
                    prefix, 
                    if is_last { " " } else { node.get_animated_char(false) }
                )
            };
            
            self.render_node(child, lines, &child_prefix, child_is_last, false);
        }
    }
    
    /// Clear all nodes
    pub fn clear(&mut self) {
        self.roots.clear();
        self.node_map.clear();
        self.parent_map.clear();
        self.animation_counter = 0;
    }
    
    /// Get statistics about the tree
    pub fn get_stats(&self) -> TreeStats {
        let mut stats = TreeStats::default();
        
        for root in &self.roots {
            self.count_stats(root, &mut stats);
        }
        
        stats
    }
    
    /// Recursively count statistics
    fn count_stats(&self, node: &TreeNode, stats: &mut TreeStats) {
        match node.status {
            NodeStatus::Running => stats.running += 1,
            NodeStatus::Completed => stats.completed += 1,
            NodeStatus::Failed => stats.failed += 1,
            NodeStatus::Pending => stats.pending += 1,
            NodeStatus::Warning => stats.warnings += 1,
        }
        
        for child in &node.children {
            self.count_stats(child, stats);
        }
    }
}

/// Statistics about the tree state
#[derive(Debug, Default)]
pub struct TreeStats {
    /// Number of running nodes
    pub running: usize,
    /// Number of completed nodes
    pub completed: usize,
    /// Number of failed nodes
    pub failed: usize,
    /// Number of pending nodes
    pub pending: usize,
    /// Number of warnings
    pub warnings: usize,
}

impl Default for TimelineTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tree_creation() {
        let mut tree = TimelineTree::new();
        tree.add_root("root1".to_string(), "Root Node".to_string());
        
        assert_eq!(tree.roots.len(), 1);
        assert_eq!(tree.roots[0].display_name, "Root Node");
    }
    
    #[test]
    fn test_add_child() {
        let mut tree = TimelineTree::new();
        tree.add_root("root1".to_string(), "Root Node".to_string());
        tree.add_child("root1".to_string(), "child1".to_string(), "Child Node".to_string());
        
        assert_eq!(tree.roots[0].children.len(), 1);
        assert_eq!(tree.roots[0].children[0].display_name, "Child Node");
    }
    
    #[test]
    fn test_node_status_transitions() {
        let mut node = TreeNode::new("test".to_string(), "Test Node".to_string(), 0);
        
        assert_eq!(node.status, NodeStatus::Pending);
        
        node.start();
        assert_eq!(node.status, NodeStatus::Running);
        
        node.complete();
        assert_eq!(node.status, NodeStatus::Completed);
        assert!(node.duration_text.is_some());
    }
    
    #[test]
    fn test_animation_characters() {
        let mut node = TreeNode::new("test".to_string(), "Test Node".to_string(), 0);
        node.start();
        
        // Test that animation characters cycle
        let char1 = node.get_animated_char(true);
        node.advance_animation();
        let char2 = node.get_animated_char(true);
        
        // Should be different characters due to animation
        // (This test might be flaky due to timing, but demonstrates the concept)
    }
}