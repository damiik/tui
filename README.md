# MCP Test Client

Elegancki klient MCP (Model Context Protocol) z interfejsem terminalowym inspirowanym filozofią Vim.

## Nowe funkcje

### 🔍 Rozszerzone debugowanie
- Szczegółowe logi procesu połączenia
- Śledzenie SSE event stream
- Debug JSON-RPC request/response
- Informacje o inicjalizacji sesji

### 🎯 Interaktywny wybór serwera
- `:mcp connect` otwiera menu wyboru
- Nawigacja: `↑`/`↓` lub `j`/`k`
- Wybór numerem: `1`, `2`, `3`...
- `Enter` - połącz, `Esc` - anuluj

## Konfiguracja

Utwórz plik `config.json` w głównym katalogu projektu:

```json
{
  "mcp_servers": [
    {
      "name": "local-server",
      "url": "http://localhost:8080"
    },
    {
      "name": "remote-server",
      "url": "https://example.com/mcp"
    }
  ]
}
```

## Uruchomienie

```bash
# Kompilacja
cargo build --release

# Uruchomienie
cargo run

# Z debugowaniem
RUST_LOG=debug cargo run
```

## Interface użytkownika

### Layout
```
┌────────────────────────────────┐
│         Output Area            │  ← Scrollowalna historia
│  🔍 Debug messages             │  ← Z emoji dla czytelności
│  📦 Response data              │
│  ❌ Error messages             │
├────────────────────────────────┤
│ MODE  Status message  Help     │  ← Status bar
├────────────────────────────────┤
│ > input buffer_               │  ← Linia wejściowa
└────────────────────────────────┘
```

### Tryby

#### **NORMAL** (cyan)
- `i` - INSERT mode
- `:` - COMMAND mode  
- `q` - szybkie wyjście
- `Ctrl+Q` - wymuszenie wyjścia
- `Ctrl+L` - czyszczenie outputu

#### **INSERT** (green)
- `ESC` - powrót do NORMAL
- `Enter` - wysłanie wejścia
- `Ctrl+W` - czyszczenie bufora
- `←`/`→`/`Home`/`End` - nawigacja

#### **COMMAND** (yellow)
```
:q, :quit              - wyjście
:clear                 - czyszczenie outputu
:echo <text>           - echo do outputu
:mcp connect           - wybór serwera (interaktywny)
:mcp connect <name>    - połączenie bezpośrednie
:mcp list              - lista skonfigurowanych serwerów
:mcp tools             - lista narzędzi MCP
:h, :help              - pomoc
```

#### **SELECT** (magenta)
Tryb wyboru serwera MCP:
- `↑`/`↓` lub `j`/`k` - nawigacja
- `1`-`9` - wybór bezpośredni
- `Enter` - potwierdź wybór
- `Esc` - anuluj

## Debugowanie połączenia MCP

Aplikacja wyświetla szczegółowe informacje o procesie połączenia:

```
🔌 Connecting to local-server at http://localhost:8080
📡 Initial response: HTTP 200
📥 Waiting for SSE endpoint...
📨 SSE event='endpoint' data='/session/abc123'
✅ Received endpoint: /session/abc123
🔗 Session endpoint: http://localhost:8080/session/abc123
🎧 Starting SSE listener on http://localhost:8080/session/abc123
📤 Sending initialize: {...}
📥 Initialize response: HTTP 200
✅ MCP session initialized
```

### Typowe problemy

**❌ POST HTTP error: 405 Method Not Allowed**
- Serwer nie przyjmuje POST na danym endpointcie
- Sprawdź czy endpoint SSE zwrócił poprawną ścieżkę sesji
- Weryfikuj logi: `📨 SSE event='endpoint'`

**⚠️ No endpoint received from server**
- Serwer nie wysłał SSE event `endpoint`
- Sprawdź format odpowiedzi serwera
- Możliwe że serwer używa innego protokołu

**Stream ended without endpoint**
- Połączenie SSE zamknęło się przed wysłaniem endpointu
- Sprawdź logi serwera MCP
- Weryfikuj czy serwer poprawnie implementuje SSE

## Przykładowa sesja

```
[NORMAL] Start
  ↓ :
[COMMAND] mcp connect
  ↓ Enter
[SELECT] 
  🔌 Select MCP server:
    → [1] local-server: http://localhost:8080
      [2] remote-server: https://example.com/mcp
  
  Use ↑↓ or j/k to navigate, Enter to connect
  ↓ Enter
[NORMAL]
  🔌 Connecting to local-server at http://localhost:8080
  📡 Initial response: HTTP 200
  📥 Waiting for SSE endpoint...
  ✅ Received endpoint: /session/abc123
  ✅ MCP session initialized
  ↓ :
[COMMAND] mcp tools
  ↓ Enter
[NORMAL]
  📤 Sending tools/list (id=2)
  📦 Available tools:
    • read_file
    • write_file
    • list_directory
```

## Architektura

### Funkcyjne wzorce
- **Immutowalne transformacje stanu** - `App::handle_event(self, event) -> Result<Self>`
- **Czyste struktury danych** - `Buffer`, `OutputLog`, `Mode`
- **Algebraiczne typy** - `Command`, `McpClientEvent` jako enums
- **Async event streaming** - tokio channels + SSE

### Moduły
```
src/
├── main.rs         # Entry point + event loop
├── app.rs          # State machine z server selection
├── mcp.rs          # MCP client (SSE + JSON-RPC)
├── command.rs      # Command parser
├── config.rs       # Configuration loader
├── mode.rs         # Modal states
├── state.rs        # Immutable buffers
├── event.rs        # Event abstraction
└── ui.rs           # Pure rendering
```

## MCP Protocol Support

Implementowane features:
- ✅ SSE transport
- ✅ JSON-RPC 2.0
- ✅ `initialize` method
- ✅ `tools/list` method
- ✅ Session endpoint discovery
- ✅ Async request/response matching
- ⏳ `tools/call` (TODO)
- ⏳ `resources/*` (TODO)
- ⏳ `prompts/*` (TODO)

## Rozszerzalność

Dodawanie nowych komend MCP:
1. Extend `Command` enum w `command.rs`
2. Add parsing logic w `Command::parse()`
3. Handle w `App::execute_command()`
4. Implement w `McpClient`

Wszystko przez kompozycję, zero dziedziczenia!