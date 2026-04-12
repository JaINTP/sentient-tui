```
  ██████ ▓█████  ███▄    █ ▄▄▄█████▓ ██▓ ▓█████  ███▄    █ ▄▄▄█████▓
▒██    ▒ ▓█   ▀  ██ ▀█   █ ▓  ██▒ ▓▒▓██▒ ▓█   ▀  ██ ▀█   █ ▓  ██▒ ▓▒
░ ▓██▄   ▒███   ▓██  ▀█ ██▒▒ ▓██░ ▒░▒██▒ ▒███   ▓██  ▀█ ██▒▒ ▓██░ ▒░
  ▒   ██▒▒▓█  ▄ ▓██▒  ▐▌██▒░ ▓██▓ ░ ░██░ ▒▓█  ▄ ▓██▒  ▐▌██▒░ ▓██▓ ░ 
▒██████▒▒░▒████▒▒██░   ▓██░  ▒██▒ ░ ░██░ ░▒████▒▒██░   ▓██░  ▒██▒ ░ 
▒ ▒▓▒ ▒ ░░░ ▒░ ░░ ▒░   ▒ ▒   ▒ ░░   ░▓   ░░ ▒░ ░░ ▒░   ▒ ▒   ▒ ░░   
░ ░▒  ░ ░ ░ ░  ░░ ░░   ░ ▒░    ░     ▒ ░  ░ ░  ░░ ░░   ░ ▒░    ░     
░  ░  ░     ░      ░   ░ ░   ░       ▒ ░    ░      ░   ░ ░   ░       
      ░     ░  ░         ░           ░      ░  ░         ░           
                                                                    
                       ▄▄▄█████▓ █    ██  ██▓               
                       ▓  ██▒ ▓▒ ██  ▓██▒▓██▒               
                       ▒ ▓██░ ▒░▓██  ▒██░▒██▒               
                       ░ ▓██▓ ░ ▓▓█  ░██░░██░               
                         ▒██▒ ░ ▒▒█████▓ ░██░               
                         ▒ ░░   ░▒▓▒ ▒ ▒ ░▓                 
                           ░    ░░▒░ ░ ░  ▒ ░               
                         ░       ░░░ ░ ░  ▒ ░               
                                   ░      ░                 
```

# Sentient TUI

A terminal user interface client for [ArtifactsMMO](https://artifactsmmo.com). Monitor and manage your characters in real-time with live game events, character stats, Grand Exchange feeds, world events, and an interactive minimap - all in your terminal.

## Features

| Feature | Description |
|---------|-------------|
| Real-time monitoring | Live character positions, levels, HP, gold, and active tasks via WebSocket |
| Character cards | Grid layout showing each character's stats, equipment, skills, and current action with animated boot sequences |
| Interactive minimap | 3x3 tile grid with sprite rendering, multi-layer support (overworld/underground/interior), and character portrait overlay |
| Grand Exchange feed | Live buy/sell orders and transaction completions in the sidebar |
| World events | Real-time alerts for spawning and despawning events |
| Economy dashboard | Total gold and gold-per-hour tracking with periodic snapshots |
| Action log | Footer log with color-coded events (combat, gathering, crafting, movement, tasks, banking, GE) |
| Image caching | Automatic download and disk caching of character skins, item icons, and map tiles |
| Performance | 60 FPS rendering with configurable tick rate; async tokio runtime for concurrent WebSocket and REST operations |
| Boot animation | Glitch effect on startup with progressive content reveal |
| Status indicators | WebSocket connection status, FPS counter, cooldown timers per character |

## Screenshots

![Sentient TUI](screenshots/scrot.png)

## Prerequisites

- **Rust toolchain** - Rust 2021 edition or later. Install from [rustup.rs](https://rustup.rs/).
- **Terminal with image protocol support** - one of:
  - [Kitty](https://sw.kovidgoyal.net/kitty/) (recommended)
  - [iTerm2](https://iterm2.com/) (macOS)
  - [WezTerm](https://wezfm.org/)
  - [Konsole](https://konsole.kde.org/) (KDE)
- **ArtifactsMMO account** - create an account at [artifactsmmo.com](https://artifactsmmo.com) and generate an API token.

## Installation

### Pre-built binaries

Download the latest pre-built binaries for **Linux**, **macOS**, and **Windows** from the [GitHub Releases](https://github.com/jaintp/sentient-tui/releases) page.

Archives include:
- `sentient-tui-*-x86_64-unknown-linux-gnu.tar.gz` (Linux x86_64)
- `sentient-tui-*-x86_64-apple-darwin.tar.gz` (macOS x86_64)
- `sentient-tui-*-aarch64-apple-darwin.tar.gz` (macOS Apple Silicon)
- `sentient-tui-*-x86_64-pc-windows-msvc.zip` (Windows x86_64)

### Arch Linux

An official `PKGBUILD` is provided in the root of the repository. You can build it using `makepkg`:

```bash
git clone https://github.com/jaintp/sentient-tui.git
cd sentient-tui
makepkg -si
```

### From source (GitHub)

```bash
git clone https://github.com/jaintp/sentient-tui.git
cd sentient-tui
cargo build --release
```

The compiled binary will be at `target/release/sentient-tui`.

## Configuration

### API token

The application requires an `ARTIFACTS_TOKEN` environment variable to connect to the ArtifactsMMO API.

**Shell export:**

```bash
export ARTIFACTS_TOKEN="your-api-token-here"
sentient-tui
```

**`.env` file** (automatically loaded from the working directory or any parent):

```
ARTIFACTS_TOKEN=your-api-token-here
```

**`.envrc` with [direnv](https://direnv.net/):**

```bash
export ARTIFACTS_TOKEN="your-api-token-here"
```

```bash
direnv allow
sentient-tui
```

### Config directory

Application configuration is stored in platform-specific directories:

| Platform | Path |
|----------|------|
| Linux | `~/.config/sentient-tui/` |
| macOS | `~/Library/Application Support/sentient-tui/` |
| Windows | `%APPDATA%\sentient-tui\` |

Override with environment variables:

```bash
export SENTIENT_TUI_CONFIG=/custom/config/path
export SENTIENT_TUI_DATA=/custom/data/path
```

## Usage

### Startup

```bash
ARTIFACTS_TOKEN=your-token sentient-tui
```

The loading screen runs while the application:
1. Fetches character data via REST (`GET /my/characters`)
2. Fetches map tiles via REST (`GET /maps`, paginated)
3. Downloads all tile sprites to the image cache
4. Establishes the WebSocket connection

The main view loads once all three complete.

### Layout

```
+-------------------------------------+-------------+
|   Character Cards Grid (80%)        |  Sidebar    |
|                                     |  (20%)      |
|  [Card] [Card] [Card] ...           | - Status    |
|  [Card] [Card] [Card] ...           | - Economy   |
|  [Card] [Card] [Card] ...           | - Events    |
|                                     | - GE Feed   |
|                                     | - Minimap   |
+-------------------------------------+-------------+
| Action Log (15%)                                  |
| [SYS] system messages                             |
| [FIGHT] combat | [GATHER] gathering               |
| [CRAFT] crafting | [GE] grand exchange            |
+---------------------------------------------------+
```

**Character cards** display:
- Character name, level, and skin portrait
- HP/Max HP progress bar
- XP progress bar
- Current action (fight, gather, craft, move, rest, task)
- Active cooldown timer
- Equipped items with icons (weapon, armor, accessories, utilities)
- Skill levels

**Sidebar** (top to bottom):
- WebSocket status indicator (connected / connecting / disconnected)
- Economy: total gold across all characters and gold-per-hour rate
- World events: active spawned events (up to 3)
- Grand Exchange feed: recent orders and completed transactions
- Minimap: 3x3 tile grid centered on the selected character

**Footer log** color codes:

| Tag | Color | Event type |
|-----|-------|------------|
| `[SYS]` | Blue | System messages (WebSocket connect/disconnect) |
| `[FIGHT]` | Red | Combat actions |
| `[GATHER]` | Green | Gathering actions |
| `[CRAFT]` | Yellow | Crafting actions |
| `[MOVE]` | Cyan | Movement |
| `[REST]` | Blue | Rest/sleep |
| `[TASK+]` / `[TASK✓]` | Magenta | New task / completed task |
| `[GE]` | Cyan/Yellow | Grand Exchange orders and transactions |
| `[BANK]` | Gray | Bank deposits and withdrawals |
| `[ACHV]` | Light Yellow | Achievements unlocked |
| `[EVT+]` / `[EVT-]` | Green/Gray | Event spawn / despawn |
| `[IMG↓]` / `[IMG✓]` / `[IMG✗]` | Cyan/Green/Red | Image download status |

### Command-line options

```bash
sentient-tui --help
```

| Flag | Default | Description |
|------|---------|-------------|
| `--tick-rate FLOAT` | `4.0` | Game tick rate in ticks per second |
| `--frame-rate FLOAT` | `60.0` | Render frame rate in frames per second |
| `--refresh-cache` | off | Wipe the local image cache before starting |

Example:

```bash
ARTIFACTS_TOKEN=your-token sentient-tui --tick-rate 2.0 --frame-rate 30.0
```

### Keybindings

| Key | Action | Description |
|-----|--------|-------------|
| `q` | Quit | Exit the application |
| `j` / `Down` / `Tab` | FocusNext | Select next character |
| `k` / `Up` / `Shift+Tab` | FocusPrev | Select previous character |
| `l` | ToggleLog | Show/hide the footer log panel |

Keybindings are customizable via the config file. See [Config directory](#config-directory) for platform-specific paths.

## Architecture

### Overview

The application uses an async, action-bus-driven architecture on top of tokio and ratatui:

```
+-----------------------------------------------------+
| WebSocket Listener                                  |
| (realtime.artifactsmmo.com, background task)        |
| Emits: account_log, online_characters, events, ...  |
+--------------------+--------------------------------+
                     |
                     v
             mpsc Action Channel
                     |
            +--------v---------+
            |  Main App Loop   |
            | - Event handling |
            | - Action routing |
            | - UI rendering   |
            +--------+---------+
                     |
       +-------------+-------------+
       |             |             |
       v             v             v
  GameState     Image Cache    Ratatui Frame
  Arc<RwLock>   (async dl +    (terminal
  characters    disk cache)     rendering)
  map tiles
  events/log

+-----------------------------------------------------+
| REST Client (one-shot fetches on startup)           |
| - GET /my/characters                                |
| - GET /maps (paginated)                             |
+-----------------------------------------------------+
```

### Core components

| Component | File | Responsibility |
|-----------|------|----------------|
| `GameState` | `src/core/game/state.rs` | Central shared state (characters, map, events, log) wrapped in `Arc<RwLock<>>` |
| `App` | `src/app.rs` | Main event loop, action routing, mode transitions (Loading -> Home) |
| `CharacterCards` | `src/ui/components/character_cards/` | Character card grid with boot animation |
| `Sidebar` | `src/ui/components/sidebar.rs` | Status, economy, events, GE feed, minimap |
| `LogPanel` | `src/ui/components/log_panel.rs` | Scrollable action log |
| `LoadingScreen` | `src/ui/components/loading_screen.rs` | Boot animation with progress gauge |
| `ImageCache` | `src/ui/image_cache.rs` | Async download + disk cache for sprites |
| `MinimapCache` | `src/ui/minimap.rs` | 3x3 tile grid renderer with per-slot StatefulProtocol |
| `network` | `src/api/network.rs` | WebSocket listener with auto-reconnect and ping keepalive |
| `rest` | `src/api/rest.rs` | One-shot REST fetches for characters and map tiles |

### Action bus

All inputs, WebSocket messages, and internal updates route through a single `mpsc` channel. Key action types:

| Category | Actions |
|----------|---------|
| Lifecycle | `Tick`, `Render`, `Quit`, `Suspend`, `Resume` |
| WebSocket | `WsConnected`, `WsDisconnected`, `WsReconnect` |
| REST | `CharactersFetched`, `MapsFetched` |
| Game events | `AccountLog`, `OnlineCharacters`, `EventSpawn`, `EventRemoved` |
| Grand Exchange | `GEOrderCreated`, `GETransactionCompleted` |
| Notifications | `AchievementUnlocked`, `Announcement` |
| System | `SystemLog` (image download events) |

Components implement a handler trait to process actions before they reach the main loop, enabling UI-local state updates (scroll position, selection state, etc.).

## Cache

### Image cache

All downloaded sprites land at:

```
~/.cache/sentient-tui/images/{category}/{code}.png
```

| Category | Source |
|----------|--------|
| `characters` | Character skin portraits |
| `items` | Equipment and inventory icons |
| `monsters` | Monster icons (task display) |
| `resources` | Resource icons (task display) |
| `maps` | Map tile sprites for the minimap |

Clear the cache to force a re-download:

```bash
sentient-tui --refresh-cache
```

Or manually:

```bash
rm -rf ~/.cache/sentient-tui/images/
```

### Logs

Application logs are written to:

```
~/.local/share/sentient-tui/sentient-tui.log
```

Path varies by OS; see [Config directory](#config-directory) for platform-specific locations.

## License

This project is licensed under the terms in the [LICENSE](LICENSE) file.

## Contributing

Contributions are welcome. Open an issue or pull request on GitHub.

## Acknowledgments

Special thanks to [Exabind](https://github.com/junkdog/exabind) project for the active component animation.

## Resources

- [ArtifactsMMO](https://artifactsmmo.com) - official game site
- [ArtifactsMMO API docs](https://docs.artifactsmmo.com) - REST and WebSocket documentation
- [Ratatui](https://ratatui.rs/) - Rust TUI framework
- [Tokio](https://tokio.rs/) - async runtime
- [tachyonfx](https://crates.io/crates/tachyonfx) - Rust TUI framework
