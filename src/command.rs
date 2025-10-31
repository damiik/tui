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
    Connect { url: String, name: String },
    McpList,
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
            ["cn", url, name] | ["connect", url, name] => Ok(Command::Connect {
                url: url.to_string(),
                name: name.to_string(),
            }),
            ["cn"] | ["connect"] => Err(CommandError::InvalidSyntax(
                "connect requires a URL and a server name".into(),
            )),
            ["mcp", "list"] => Ok(Command::McpList),
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
    fn test_connect_command() {
        assert_eq!(
            Command::parse("connect http://localhost:8080/sse pcbvi-mcp-server"),
            Ok(Command::Connect {
                url: "http://localhost:8080/sse".into(),
                name: "pcbvi-mcp-server".into()
            })
        );
        assert_eq!(
            Command::parse("cn http://localhost:8080/sse pcbvi-mcp-server"),
            Ok(Command::Connect {
                url: "http://localhost:8080/sse".into(),
                name: "pcbvi-mcp-server".into()
            })
        );
    }

    fn test_connect_without_args() {
        assert!(matches!(
            Command::parse("connect"),
            Err(CommandError::InvalidSyntax(_))
        ));
        assert!(matches!(
            Command::parse("cn"),
            Err(CommandError::InvalidSyntax(_))
        ));
    }

    #[test]
    fn test_mcp_list_command() {
        assert_eq!(Command::parse("mcp list"), Ok(Command::McpList));
    }
}
