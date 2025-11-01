use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Unknown command: {0}")]
    Unknown(String),
    #[error("Invalid syntax: {0}")]
    InvalidSyntax(String),
    #[error("Empty command")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Quit,
    Clear,
    Echo(String),
    Help,
    McpConnect(Option<String>),
    McpList,
    McpTools,
    McpRun(Option<String>),
    McpStatus, // New: show connection status
    Mouse(bool),
}

impl Command {
    /// Pure parser: &str → Result<Command, CommandError>
    pub fn parse(input: &str) -> Result<Self, CommandError> {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Err(CommandError::Empty);
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();

        match parts.as_slice() {
            ["q"] | ["quit"] => Ok(Command::Quit),
            ["clear"] => Ok(Command::Clear),
            ["h"] | ["help"] => Ok(Command::Help),
            ["echo", rest @ ..] => {
                if rest.is_empty() {
                    Err(CommandError::InvalidSyntax(
                        "echo requires an argument".into(),
                    ))
                } else {
                    Ok(Command::Echo(rest.join(" ")))
                }
            }
            ["mcp", "cn"] | ["mcp", "connect"] => Ok(Command::McpConnect(None)),
            ["mcp", "cn", name] | ["mcp", "connect", name] => {
                Ok(Command::McpConnect(Some(name.to_string())))
            }
            ["mcp", "list"] => Ok(Command::McpList),
            ["mcp", "tools"] => Ok(Command::McpTools),
            ["mcp", "run"] => Ok(Command::McpRun(None)),
            ["mcp", "run", name] => Ok(Command::McpRun(Some(name.to_string()))),
            ["mcp", "status"] => Ok(Command::McpStatus),
            ["mouse", "on"] => Ok(Command::Mouse(true)),
            ["mouse", "off"] => Ok(Command::Mouse(false)),
            [cmd, ..] => Err(CommandError::Unknown(cmd.to_string())),
            [] => unreachable!(), // Already handled empty case
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Tests: Property-based validation
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quit_commands() {
        assert_eq!(Command::parse("q"), Ok(Command::Quit));
        assert_eq!(Command::parse("quit"), Ok(Command::Quit));
        assert_eq!(Command::parse("  q  "), Ok(Command::Quit));
    }

    #[test]
    fn test_clear_command() {
        assert_eq!(Command::parse("clear"), Ok(Command::Clear));
    }

    #[test]
    fn test_echo_command() {
        assert_eq!(
            Command::parse("echo hello world"),
            Ok(Command::Echo("hello world".into()))
        );
    }

    #[test]
    fn test_mcp_connect_command() {
        assert_eq!(
            Command::parse("mcp connect pcbvi-mcp-server"),
            Ok(Command::McpConnect(Some("pcbvi-mcp-server".into())))
        );
        assert_eq!(Command::parse("mcp connect"), Ok(Command::McpConnect(None)));
    }

    #[test]
    fn test_mcp_tools_command() {
        assert_eq!(Command::parse("mcp tools"), Ok(Command::McpTools));
    }

    #[test]
    fn test_mcp_run_command() {
        assert_eq!(Command::parse("mcp run"), Ok(Command::McpRun(None)));
        assert_eq!(
            Command::parse("mcp run get_view_state"),
            Ok(Command::McpRun(Some("get_view_state".into())))
        );
    }

    #[test]
    fn test_mouse_commands() {
        assert_eq!(Command::parse("mouse on"), Ok(Command::Mouse(true)));
        assert_eq!(Command::parse("mouse off"), Ok(Command::Mouse(false)));
    }
}