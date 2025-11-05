# TUI MCP Client

A terminal-based UI client for the Model Context Protocol (MCP), inspired by Vim's modal interface. It allows for interactive communication with MCP-compliant servers, tool execution, and context management directly from the command line.

## Features

- **Modal Interface**: Vim-like `NORMAL`, `INSERT`, and `COMMAND` modes for efficient, keyboard-driven interaction.
- **MCP Communication**: Connects to MCP servers via Server-Sent Events (SSE) and JSON-RPC.
- **Interactive Selection Menus**: Easily select servers and tools from dynamic lists.
- **Command System**: A rich set of commands for controlling the application, managing connections, and interacting with tools.
- **Command Completion**: Press `Tab` in `COMMAND` mode to auto-complete commands, tool names, and server names.
- **Command History**: Navigate through previously executed commands using the `Up` and `Down` arrow keys.
- **Tool Inspection**: View detailed information about available tools, including descriptions and input schemas.
- **Dynamic Layout**: The UI adapts to different terminal sizes.
- **Mouse Support**: Optional mouse capture for scrolling and other interactions.
- **Configurable**: Define MCP server endpoints in an external `config.json` file.

## Configuration

Create a `config.json` file in the root of the project directory to define the MCP servers you want to connect to.

**`config.json` format:**
```json
{
  "mcp_servers": [
    {
      "name": "local-dev",
      "url": "http://localhost:8080/sse"
    },
    {
      "name": "staging-server",
      "url": "https://mcp.staging.example.com"
    }
  ]
}
```

## How to Run

1.  **Build the project:**
    ```bash
    cargo build --release
    ```

2.  **Run the application:**
    ```bash
    cargo run
    ```

## User Interface

The UI is split into three main sections:

```
┌───────────────────────────────────────────────────┐
│ 📦 Available tools:                               │  ← Output Area
│   • read_file: Reads a file from the filesystem.  │  (Scrollable history of commands and responses)
│   • list_directory: Lists files in a directory.   │
│                                                   │
├───────────────────────────────────────────────────┤
│ NORMAL | Ready                                    │  ← Status Bar
├───────────────────────────────────────────────────┤
│ :mcp tools_                                       │  ← Input/Command Line
└───────────────────────────────────────────────────┘
```

### Modes

-   **NORMAL** (`Cyan`): The default mode for navigation and entering other modes.
-   **INSERT** (`Green`): For typing input to be sent to the server (currently echoes back).
-   **COMMAND** (`Yellow`): For entering commands to control the application (e.g., `:q`, `:mcp connect`).
-   **SELECT** (`Magenta`): An interactive mode for selecting a server or tool from a list.

### Keybindings

| Mode      | Key(s)                  | Action                                           |
| :-------- | :---------------------- | :----------------------------------------------- |
| **NORMAL**  | `i`                     | Enter `INSERT` mode.                             |
|           | `:`                     | Enter `COMMAND` mode.                            |
|           | `q`                     | Quit the application.                            |
|           | `k` / `Up`              | Scroll output up.                                |
|           | `j` / `Down`            | Scroll output down.                              |
|           | `PageUp` / `PageDown`   | Scroll output by a full page.                    |
|           | `End`                   | Jump to the bottom of the output (enables autoscroll). |
|           | `Ctrl+L`                | Clear the output area.                           |
| **INSERT**  | `Esc`                   | Return to `NORMAL` mode.                         |
|           | `Enter`                 | Send the input.                                  |
|           | `Backspace`             | Delete character before the cursor.              |
|           | `Left`/`Right`/`Home`/`End` | Navigate the input line.                     |
| **COMMAND** | `Esc`                   | Return to `NORMAL` mode.                         |
|           | `Enter`                 | Execute the command or apply completion.         |
|           | `Tab`                   | Trigger command/argument completion.             |
|           | `Up`/`Down`             | Navigate command history or completion list.     |
|           | `Backspace`             | Delete character before the cursor.              |
|           | `Left`/`Right`/`Home`/`End` | Navigate the command line.                   |
| **SELECT**  | `k` / `Up`              | Move selection up.                               |
|           | `j` / `Down`            | Move selection down.                             |
|           | `Enter`                 | Confirm selection.                               |
|           | `Esc`                   | Cancel selection and return to `NORMAL` mode.    |

## Commands

Commands are entered in `COMMAND` mode, prefixed with `:`.

| Command                             | Alias       | Description                                                              |
| :---------------------------------- | :---------- | :----------------------------------------------------------------------- |
| `:q`, `:quit`                       |             | Exit the application.                                                    |
| `:clear`                            |             | Clear the output area.                                                   |
| `:echo <text>`                      |             | Print `<text>` to the output area.                                       |
| `:h`, `:help`                       |             | Show the help message with all available commands.                       |
| `:mouse on` / `:mouse off`          |             | Enable or disable mouse capture.                                         |
| `:mcp list`                         |             | List all configured MCP servers from `config.json`.                      |
| `:mcp connect [name]`               | `:mcp cn`   | Connect to an MCP server. Opens an interactive menu if `[name]` is omitted. |
| `:mcp status`                       |             | Show the current MCP connection status and number of loaded tools.       |
| `:mcp tools`                        |             | List all available tools from the connected MCP server.                  |
| `:mcp tool <tool_name>`             |             | Show a detailed description of `<tool_name>`, including its input schema. |
| `:mcp run [tool_name] [args...]`    |             | Execute a tool. Opens an interactive menu if `[tool_name]` is omitted.   |

## Architecture

The application follows a functional, event-driven architecture inspired by Elm.

-   **`main.rs`**: The entry point, responsible for setting up the terminal, initializing the `App`, and running the main event loop.
-   **`app.rs`**: The core state machine. It holds all application state and handles state transitions in response to events.
-   **`ui.rs`**: Contains all rendering logic. It is a pure function that maps the `App` state to the terminal frame.
-   **`event.rs`**: Defines the main event loop and abstracts away terminal events.
-   **`mcp.rs`**: The MCP client, responsible for handling SSE connections, sending JSON-RPC requests, and receiving responses.
-   **`command.rs`**: The command parser, which validates and translates command strings into structured `Command` enums.
-   **`config.rs`**: Handles loading and parsing the `config.json` file.
-   **`state.rs`**: Defines simple, immutable data structures for buffers and logs.
-   **`mode.rs`**: Defines the different application modes (`Normal`, `Insert`, `Command`).
-   **`completion.rs`**: Implements the logic for command completion and history.
