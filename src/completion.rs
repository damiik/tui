// ============================================================================
// src/completion.rs - Vim-style command completion system
// ============================================================================

use std::collections::HashMap;

// ============================================================================
// Integration with App State
// ============================================================================

/// Command buffer state with completion support
#[derive(Debug, Clone)]
pub struct CommandBufferState {
    /// Current command text
    pub content: String,
    /// Cursor position
    pub cursor: usize,
    /// Active completion popup
    pub completion: Option<CompletionResult>,
    /// History navigation state
    pub history_index: Option<usize>,
    /// Original text before history navigation
    pub saved_text: Option<String>,
}

impl CommandBufferState {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor: 0,
            completion: None,
            history_index: None,
            saved_text: None,
        }
    }

    pub fn with_char(mut self, c: char) -> Self {
        self.content.insert(self.cursor, c);
        self.cursor += 1;
        self.completion = None; // Clear completion on edit
        self.history_index = None;
        self
    }

    pub fn delete_char(mut self) -> Self {
        if self.cursor > 0 && !self.content.is_empty() {
            self.content.remove(self.cursor - 1);
            self.cursor -= 1;
            self.completion = None;
            self.history_index = None;
        }
        self
    }

    pub fn move_left(mut self) -> Self {
        self.cursor = self.cursor.saturating_sub(1);
        self.completion = None;
        self
    }

    pub fn move_right(mut self) -> Self {
        if self.cursor < self.content.len() {
            self.cursor += 1;
        }
        self.completion = None;
        self
    }

    pub fn move_start(mut self) -> Self {
        self.cursor = 0;
        self.completion = None;
        self
    }

    pub fn move_end(mut self) -> Self {
        self.cursor = self.content.len();
        self.completion = None;
        self
    }

    pub fn clear(mut self) -> Self {
        self.content.clear();
        self.cursor = 0;
        self.completion = None;
        self.history_index = None;
        self.saved_text = None;
        self
    }

    /// Activate completion popup
    pub fn with_completion(mut self, result: CompletionResult) -> Self {
        if !result.is_empty() {
            self.completion = Some(result);
        }
        self
    }

    /// Apply selected completion
    pub fn apply_completion(mut self) -> Self {
        if let Some(ref comp) = self.completion {
            if let Some(text) = comp.selected_text() {
                // Replace the word being completed
                let parts: Vec<&str> = self.content.split_whitespace().collect();
                
                if parts.is_empty() {
                    self.content = text.to_string();
                } else if self.content.ends_with(' ') {
                    self.content.push_str(text);
                } else {
                    // Replace last word
                    let last_space = self.content.rfind(' ');
                    match last_space {
                        Some(pos) => {
                            self.content.truncate(pos + 1);
                            self.content.push_str(text);
                        }
                        None => {
                            self.content = text.to_string();
                        }
                    }
                }
                
                self.cursor = self.content.len();
                self.completion = None;
            }
        }
        self
    }

    pub fn set_text(mut self, text: String) -> Self {
        self.cursor = text.len();
        self.content = text;
        self.completion = None;
        self
    }
}

impl Default for CommandBufferState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Completion Context
// ============================================================================


/// Immutable completion context - pure functional data structure
#[derive(Debug, Clone)]
pub struct CompletionContext {
    /// Available command templates with their argument schemas
    commands: HashMap<String, CommandTemplate>,
    /// Dynamic completion lists (servers, tools, etc.)
    lists: HashMap<String, Vec<String>>,
    /// Command history for cycling through previous commands
    history: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CommandTemplate {
    pub name: String,
    pub description: String,
    pub args: Vec<ArgTemplate>,
}

#[derive(Debug, Clone)]
pub struct ArgTemplate {
    pub name: String,
    pub required: bool,
    pub completion_list: Option<String>, // Reference to a completion list
}

/// Completion result with candidates
#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub candidates: Vec<CompletionCandidate>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct CompletionCandidate {
    pub text: String,
    pub description: Option<String>,
}

impl CompletionContext {
    pub fn new() -> Self {
        let mut commands = HashMap::new();
        
        // Register all available commands
        commands.insert("q".to_string(), CommandTemplate {
            name: "q".to_string(),
            description: "Quit application".to_string(),
            args: vec![],
        });
        
        commands.insert("quit".to_string(), CommandTemplate {
            name: "quit".to_string(),
            description: "Quit application".to_string(),
            args: vec![],
        });
        
        commands.insert("clear".to_string(), CommandTemplate {
            name: "clear".to_string(),
            description: "Clear output".to_string(),
            args: vec![],
        });
        
        commands.insert("echo".to_string(), CommandTemplate {
            name: "echo".to_string(),
            description: "Echo text to output".to_string(),
            args: vec![
                ArgTemplate {
                    name: "text".to_string(),
                    required: true,
                    completion_list: None,
                }
            ],
        });
        
        commands.insert("help".to_string(), CommandTemplate {
            name: "help".to_string(),
            description: "Show help".to_string(),
            args: vec![],
        });
        
        commands.insert("h".to_string(), CommandTemplate {
            name: "h".to_string(),
            description: "Show help".to_string(),
            args: vec![],
        });
        
        commands.insert("mouse".to_string(), CommandTemplate {
            name: "mouse".to_string(),
            description: "Enable/disable mouse capture".to_string(),
            args: vec![
                ArgTemplate {
                    name: "state".to_string(),
                    required: true,
                    completion_list: Some("mouse_states".to_string()),
                }
            ],
        });
        
        commands.insert("mcp".to_string(), CommandTemplate {
            name: "mcp".to_string(),
            description: "MCP commands".to_string(),
            args: vec![
                ArgTemplate {
                    name: "subcommand".to_string(),
                    required: true,
                    completion_list: Some("mcp_subcommands".to_string()),
                }
            ],
        });

        let mut lists = HashMap::new();
        
        // Static completion lists
        lists.insert("mouse_states".to_string(), vec![
            "on".to_string(),
            "off".to_string(),
        ]);
        
        lists.insert("mcp_subcommands".to_string(), vec![
            "list".to_string(),
            "connect".to_string(),
            "cn".to_string(),
            "tools".to_string(),
            "tool".to_string(),
            "run".to_string(),
            "status".to_string(),
        ]);

        Self {
            commands,
            lists,
            history: Vec::new(),
        }
    }

    /// Pure function: registers a dynamic completion list
    pub fn with_list(mut self, name: String, items: Vec<String>) -> Self {
        self.lists.insert(name, items);
        self
    }

    /// Pure function: adds command to history
    pub fn with_history_entry(mut self, command: String) -> Self {
        // Remove duplicate if exists
        self.history.retain(|h| h != &command);
        // Add to end (most recent)
        self.history.push(command);
        // Keep only last 100 commands
        if self.history.len() > 100 {
            self.history.drain(0..1);
        }
        self
    }

    /// Pure function: compute completions for given input
    pub fn complete(&self, input: &str) -> CompletionResult {
        let trimmed = input.trim_start();
        
        if trimmed.is_empty() {
            // No input - show all commands
            return self.complete_all_commands();
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        
        match parts.as_slice() {
            // Single token - complete command name
            [prefix] if !input.ends_with(' ') => {
                self.complete_command_name(prefix)
            }
            
            // Command with space - complete arguments
            [cmd] if input.ends_with(' ') => {
                self.complete_first_arg(cmd)
            }
            
            // Multi-part command
            [cmd, args @ ..] => {
                self.complete_command_args(cmd, args, input.ends_with(' '))
            }
            
            _ => CompletionResult::empty(),
        }
    }

    fn complete_all_commands(&self) -> CompletionResult {
        let mut candidates: Vec<CompletionCandidate> = self.commands
            .values()
            .map(|cmd| CompletionCandidate {
                text: cmd.name.clone(),
                description: Some(cmd.description.clone()),
            })
            .collect();

        candidates.sort_by(|a, b| a.text.cmp(&b.text));

        CompletionResult {
            candidates,
            selected: 0,
        }
    }

    fn complete_command_name(&self, prefix: &str) -> CompletionResult {
        let prefix_lower = prefix.to_lowercase();
        
        let mut candidates: Vec<CompletionCandidate> = self.commands
            .values()
            .filter(|cmd| cmd.name.starts_with(&prefix_lower))
            .map(|cmd| CompletionCandidate {
                text: cmd.name.clone(),
                description: Some(cmd.description.clone()),
            })
            .collect();

        candidates.sort_by(|a, b| a.text.cmp(&b.text));

        CompletionResult {
            candidates,
            selected: 0,
        }
    }

    fn complete_first_arg(&self, cmd: &str) -> CompletionResult {
        if let Some(template) = self.commands.get(cmd) {
            if let Some(first_arg) = template.args.first() {
                if let Some(list_name) = &first_arg.completion_list {
                    return self.complete_from_list(list_name, "");
                }
            }
        }
        CompletionResult::empty()
    }

    fn complete_command_args(
        &self,
        cmd: &str,
        args: &[&str],
        ends_with_space: bool,
    ) -> CompletionResult {
        // Special handling for MCP subcommands
        if cmd == "mcp" && !args.is_empty() {
            return self.complete_mcp_args(args, ends_with_space);
        }

        // Generic argument completion
        if let Some(template) = self.commands.get(cmd) {
            let arg_index = if ends_with_space {
                args.len()
            } else {
                args.len().saturating_sub(1)
            };

            if let Some(arg_template) = template.args.get(arg_index) {
                if let Some(list_name) = &arg_template.completion_list {
                    let prefix = if ends_with_space { "" } else { args.last().unwrap_or(&"") };
                    return self.complete_from_list(list_name, prefix);
                }
            }
        }

        CompletionResult::empty()
    }

    fn complete_mcp_args(&self, args: &[&str], ends_with_space: bool) -> CompletionResult {
        match args {
            // First arg after "mcp" - subcommand
            [prefix] if !ends_with_space => {
                self.complete_from_list("mcp_subcommands", prefix)
            }
            
            // After "mcp connect" or "mcp cn" - server name
            ["connect"] | ["cn"] if ends_with_space => {
                self.complete_from_list("mcp_servers", "")
            }
            ["connect", prefix] | ["cn", prefix] if !ends_with_space => {
                self.complete_from_list("mcp_servers", prefix)
            }
            
            // FIXED: After "mcp tool" - tool name for detailed description
            ["tool"] if ends_with_space => {
                self.complete_from_list("mcp_tools", "")
            }
            ["tool", prefix] if !ends_with_space => {
                self.complete_from_list("mcp_tools", prefix)
            }
            
            // After "mcp run" - tool name
            ["run"] if ends_with_space => {
                self.complete_from_list("mcp_tools", "")
            }
            ["run", prefix] if !ends_with_space => {
                self.complete_from_list("mcp_tools", prefix)
            }
            ["run", _tool, _args @ ..] => {
                // TODO: Tool-specific argument completion
                CompletionResult::empty()
            }
            
            _ => CompletionResult::empty(),
        }
    }

    fn complete_from_list(&self, list_name: &str, prefix: &str) -> CompletionResult {
        if let Some(items) = self.lists.get(list_name) {
            let prefix_lower = prefix.to_lowercase();
            
            let candidates: Vec<CompletionCandidate> = items
                .iter()
                .filter(|item| item.to_lowercase().starts_with(&prefix_lower))
                .map(|item| CompletionCandidate {
                    text: item.clone(),
                    description: None,
                })
                .collect();

            return CompletionResult {
                candidates,
                selected: 0,
            };
        }
        
        CompletionResult::empty()
    }

    /// Navigate through command history
    pub fn history_up(&self, current_index: Option<usize>) -> Option<(String, usize)> {
        if self.history.is_empty() {
            return None;
        }

        let index = match current_index {
            None => self.history.len() - 1,
            Some(i) if i > 0 => i - 1,
            Some(i) => i,
        };

        Some((self.history[index].clone(), index))
    }

    pub fn history_down(&self, current_index: Option<usize>) -> Option<(String, usize)> {
        if self.history.is_empty() {
            return None;
        }

        match current_index {
            Some(i) if i < self.history.len() - 1 => {
                Some((self.history[i + 1].clone(), i + 1))
            }
            _ => None,
        }
    }
}

impl CompletionResult {
    pub fn empty() -> Self {
        Self {
            candidates: Vec::new(),
            selected: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Pure function: navigate to next candidate
    pub fn next(mut self) -> Self {
        if !self.candidates.is_empty() {
            self.selected = (self.selected + 1) % self.candidates.len();
        }
        self
    }

    /// Pure function: navigate to previous candidate
    pub fn prev(mut self) -> Self {
        if !self.candidates.is_empty() {
            self.selected = if self.selected == 0 {
                self.candidates.len() - 1
            } else {
                self.selected - 1
            };
        }
        self
    }

    pub fn selected_text(&self) -> Option<&str> {
        self.candidates.get(self.selected).map(|c| c.text.as_str())
    }
}

impl Default for CompletionContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// USAGE SUMMARY
// ============================================================================
//
// This module provides Vim-style command completion with:
// - Tab completion for commands and arguments
// - Up/Down navigation through completion candidates
// - Command history navigation (Up/Down when no completion active)
// - Dynamic completion lists for servers, tools, etc.
// - Context-aware argument completion
//
// Key features:
// - Pure functional design - all transformations are immutable
// - Signal-based completion: lists are registered dynamically based on app state
// - History management with duplicate removal
// - Multi-level completion (commands, subcommands, arguments)
//
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_empty() {
        let ctx = CompletionContext::new();
        let result = ctx.complete("");
        assert!(!result.is_empty());
        assert!(result.candidates.iter().any(|c| c.text == "quit"));
    }

    #[test]
    fn test_complete_prefix() {
        let ctx = CompletionContext::new();
        let result = ctx.complete("q");
        assert!(!result.is_empty());
        assert!(result.candidates.iter().all(|c| c.text.starts_with('q')));
    }

    #[test]
    fn test_complete_mcp_subcommand() {
        let ctx = CompletionContext::new();
        let result = ctx.complete("mcp ");
        assert!(!result.is_empty());
        assert!(result.candidates.iter().any(|c| c.text == "connect"));
    }

    #[test]
    fn test_complete_with_dynamic_list() {
        let ctx = CompletionContext::new()
            .with_list("mcp_servers".to_string(), vec![
                "server1".to_string(),
                "server2".to_string(),
            ]);
        
        let result = ctx.complete("mcp connect ");
        assert_eq!(result.len(), 2);
        assert!(result.candidates.iter().any(|c| c.text == "server1"));
    }

    #[test]
    fn test_history_navigation() {
        let ctx = CompletionContext::new()
            .with_history_entry("echo hello".to_string())
            .with_history_entry("mcp list".to_string());

        let (cmd, idx) = ctx.history_up(None).unwrap();
        assert_eq!(cmd, "mcp list");
        assert_eq!(idx, 1);

        let (cmd, idx) = ctx.history_up(Some(idx)).unwrap();
        assert_eq!(cmd, "echo hello");
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_completion_navigation() {
        let result = CompletionResult {
            candidates: vec![
                CompletionCandidate { text: "a".to_string(), description: None },
                CompletionCandidate { text: "b".to_string(), description: None },
                CompletionCandidate { text: "c".to_string(), description: None },
            ],
            selected: 0,
        };

        let result = result.next();
        assert_eq!(result.selected, 1);

        let result = result.next();
        assert_eq!(result.selected, 2);

        let result = result.next();
        assert_eq!(result.selected, 0); // Wrap around

        let result = result.prev();
        assert_eq!(result.selected, 2); // Wrap around backwards
    }
}