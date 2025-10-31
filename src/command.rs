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
}

impl Command {
    /// Pure parser: &str → Result<Command, CommandError>
    /// Functional transformation with explicit error handling
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
        assert_eq!(
            Command::parse("echo    multiple   spaces"),
            Ok(Command::Echo("multiple   spaces".into()))
        );
    }

    #[test]
    fn test_echo_without_args() {
        assert!(matches!(
            Command::parse("echo"),
            Err(CommandError::InvalidSyntax(_))
        ));
    }

    #[test]
    fn test_unknown_command() {
        assert!(matches!(
            Command::parse("unknown"),
            Err(CommandError::Unknown(_))
        ));
    }

    #[test]
    fn test_empty_command() {
        assert!(matches!(Command::parse(""), Err(CommandError::Empty)));
        assert!(matches!(Command::parse("   "), Err(CommandError::Empty)));
    }

    #[test]
    fn test_help_command() {
        assert_eq!(Command::parse("help"), Ok(Command::Help));
        assert_eq!(Command::parse("h"), Ok(Command::Help));
    }

    #[test]
    fn test_mcp_connect_command() {
        assert_eq!(
            Command::parse("mcp connect pcbvi-mcp-server"),
            Ok(Command::McpConnect(Some("pcbvi-mcp-server".into())))
        );
        assert_eq!(
            Command::parse("mcp cn pcbvi-mcp-server"),
            Ok(Command::McpConnect(Some("pcbvi-mcp-server".into())))
        );
        assert_eq!(
            Command::parse("mcp connect"),
            Ok(Command::McpConnect(None))
        );
        assert_eq!(Command::parse("mcp cn"), Ok(Command::McpConnect(None)));
    }

    #[test]
    fn test_mcp_list_command() {
        assert_eq!(Command::parse("mcp list"), Ok(Command::McpList));
    }

    #[test]
    fn test_mcp_tools_command() {
        assert_eq!(Command::parse("mcp tools"), Ok(Command::McpTools));
    }
}
