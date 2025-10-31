# MCP Test Client

Elegancki klient MCP z interfejsem terminalowym inspirowanym filozofią Vim.

## Filozofia projektu

### Funkcyjne wzorce programowania
- **Immutability** - wszystkie transformacje stanu zwracają nowe wartości
- **Pure functions** - rendering UI i parsowanie komend bez efektów ubocznych
- **Algebraiczne typy danych** - `Mode`, `Command`, `Event` jako enums
- **Kompozycja** - moduły łączą się przez czyste interfejsy

### Architektura modułowa

```
src/
├── main.rs         # Entry point, terminal setup/teardown
├── lib.rs          # Module exports
├── app.rs          # Core state machine: App × Event → App
├── mode.rs         # Modal states (Normal/Insert/Command)
├── state.rs        # Immutable data structures (Buffer, OutputLog)
├── command.rs      # Command parser with algebraic errors
├── event.rs        # Event stream abstraction
└── ui.rs           # Pure rendering logic
```

## Budowanie i uruchamianie

```bash
# Kompilacja
cargo build --release

# Uruchomienie
cargo run

# Testy
cargo test
```

## Interface użytkownika

### Layout
```
┌────────────────────────────────┐
│         Output Area            │  ← Scrollowalna historia wyjścia
│                                │
│                                │
├────────────────────────────────┤
│ MODE  Status message  Help     │  ← Status bar z kontekstem
├────────────────────────────────┤
│ > input buffer_                │  ← Linia wejściowa z kursorem
└────────────────────────────────┘
```

### Tryby (Modal Editing)

#### **NORMAL** (niebieski)
Domyślny tryb nawigacji.

**Klawisze:**
- `i` - przejście do trybu INSERT
- `:` - przejście do trybu COMMAND  
- `q` - szybkie wyjście
- `Ctrl+Q` - wymuszenie wyjścia
- `Ctrl+L` - czyszczenie output

#### **INSERT** (zielony)
Tryb wprowadzania danych do wysłania.

**Klawisze:**
- `ESC` - powrót do NORMAL
- `Enter` - wysłanie wejścia
- `Ctrl+W` - czyszczenie bufora wejściowego
- `←` / `→` - nawigacja kursorem
- `Home` / `End` - skok do początku/końca
- `Backspace` - usuwanie znaku

#### **COMMAND** (żółty)
Tryb poleceń systemowych.

**Dostępne komendy:**
- `:q`, `:quit` - wyjście z aplikacji
- `:clear` - wyczyszczenie outputu
- `:echo <text>` - echo tekstu do output
- `:help` - wyświetlenie pomocy

**Klawisze:**
- `ESC` - anulowanie, powrót do NORMAL
- `Enter` - wykonanie komendy
- `Backspace` - usuwanie znaku

## Przykładowa sesja

```
[NORMAL] Uruchomienie programu
  ↓ i
[INSERT] Wpisz: hello world
  ↓ Enter
[INSERT] → hello world
         ← Echo: hello world
  ↓ ESC
[NORMAL]
  ↓ :
[COMMAND] Wpisz: echo test
  ↓ Enter
[NORMAL] test
  ↓ :
[COMMAND] Wpisz: q
  ↓ Enter
[EXIT]
```

## Funkcyjne property-testing

Wszystkie moduły posiadają testy jednostkowe weryfikujące:
- Parsowanie komend (exhaustive pattern matching)
- Transformacje bufora (immutability)
- Przejścia między trybami (state machine validity)

```bash
cargo test
```

## Zaawansowane techniki

### 1. State Machine jako czysta funkcja
```rust
fn handle_event(self, event: Event) -> Result<Self>
```
Każda transformacja stanu to nowa wartość - brak mutacji.

### 2. Algebraiczne typy błędów
```rust
enum CommandError {
    Unknown(String),
    InvalidSyntax(String),
    Empty,
}
```
Błędy jako wartości, nie wyjątki.

### 3. Kompozycja renderingu
```rust
render(frame, app) = 
    render_output(frame, app, area) 
    ∘ render_status(frame, app, area)
    ∘ render_input(frame, app, area)
```

### 4. Bounded data structures
```rust
const MAX_LOG_LINES: usize = 1000;
// OutputLog automatycznie utrzymuje limit
```

## Proxy pointers

Projekt używa sygnałów dojrzałości ekosystemu:

1. **ratatui** - stabilny protokół TUI, aktywnie rozwijany
2. **crossterm** - cross-platform terminal manipulation
3. **Brak `unsafe`** - 100% bezpieczny kod bez UB
4. **Testy unit** - każdy moduł testowany izolacyjnie
5. **No `panic!`** - wszystkie błędy przez `Result<T, E>`

## Rozszerzalność

Architektura pozwala łatwo dodać:
- Nowe komendy (extend `Command` enum)
- Nowe tryby (extend `Mode` enum)
- Protokół MCP (add `mcp` module)
- Historię komend (add `History` state)
- Autocomplete (extend command parser)

Wszystko przez kompozycję, nie dziedziczenie.








################################# PODSUMOWANIE ###########################################
# MCP Test Client

Terminal-based MCP (Model Context Protocol) client z interfejsem inspirowanym Vim.

## Architektura

Projekt wykorzystuje funkcyjne wzorce projektowe i modularne podejście:

### Moduły

- **`app`** - Stan aplikacji z niemutowalną logiką przejść stanów
- **`mode`** - Tryby edytora (Normal, Insert, Command) jako typ sum
- **`event`** - Strumień zdarzeń z klawiaturą
- **`ui`** - Czysta funkcja renderowania UI
- **`command`** - Parser komend z algebraicznym podejściem do błędów
- **`mcp`** - Protokół MCP z funkcyjnym API

### Tryby (Modal Editing)

1. **NORMAL** (domyślny)
   - `i` - przejście do trybu INSERT
   - `:` - przejście do trybu COMMAND
   - `Ctrl+Q` - wyjście z programu

2. **INSERT**
   - Wprowadzanie tekstu
   - `Enter` - wysłanie zapytania
   - `Esc` - powrót do NORMAL
   - Strzałki - poruszanie kursorem

3. **COMMAND**
   - `:q` lub `:quit` - wyjście
   - `:connect <server>` - połączenie z serwerem MCP
   - `:list` - lista dostępnych narzędzi
   - `:call <tool> <args>` - wywołanie narzędzia
   - `:clear` - czyszczenie output
   - `Esc` - anulowanie, powrót do NORMAL


## Zaawansowane techniki funkcyjne

1. **Niemutowalność** - Stan aplikacji jest przekazywany przez transformacje, nie mutowany
2. **Pure functions** - UI rendering i parsowanie komend są czystymi funkcjami
3. **Algebraiczne typy danych** - Użycie enum z pattern matching
4. **Composition over inheritance** - Moduły kompozytują funkcjonalność
5. **Error handling jako typ** - Błędy jako wartości (`Result`, `Option`)

## Filozofia projektu

Kod traktuje proxy pointers jako wskaźniki dojrzałości ekosystemu:
- Użycie `ratatui` (stabilny protokół TUI)
- Wzorce z funkcyjnego programowania (immutability, pure functions)
- Modularność umożliwiająca izolowane testy
- Brak ukrytych stanów czy efektów ubocznych
