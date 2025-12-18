# Hydra Mail Architecture (v0.1.0)

> Complete implementation guide and architectural documentation

## Table of Contents

1. [Overview](#overview)
2. [Project Structure](#project-structure)
3. [Core Components](#core-components)
4. [Data Flow](#data-flow)
5. [Message Protocol](#message-protocol)
6. [Channel System](#channel-system)
7. [Configuration System](#configuration-system)
8. [TOON Encoding](#toon-encoding)
9. [Daemon Implementation](#daemon-implementation)
10. [CLI Commands](#cli-commands)
11. [Testing](#testing)
12. [Performance Characteristics](#performance-characteristics)
13. [Known Gaps](#known-gaps)

## Overview

Hydra Mail is a lightweight (1,310 LOC), in-memory pub/sub messaging system designed for local AI agent collaboration. It provides project-scoped communication channels with minimal latency using Unix Domain Sockets and TOON (Token-Oriented Object Notation) encoding.

### Key Capabilities

- **Project-Aware**: Each project gets isolated `.hydra/` configuration with unique UUID
- **Daemon Mode**: Optional persistent daemon shares channel state across processes
- **TOON Protocol**: 30-60% token savings vs JSON for AI agent messages
- **Replay Buffer**: 100-message history per channel for late-joining subscribers
- **Zero Network**: Unix Domain Sockets only, no TCP exposure
- **Fast**: <5ms latency, 1M+ events/sec (claimed, needs verification)

### Architecture Philosophy

1. **Local-Only**: No distributed coordination, single-host pub/sub
2. **Ephemeral by Default**: In-memory only (sled durability is opt-in feature)
3. **Project-Scoped**: UUID isolation prevents cross-project interference
4. **TOON-First**: No JSON fallback in v0.1.0

## Project Structure

```
hydra-tools/                        # Monorepo root
├── flake.nix                       # Nix build (crane + rust-overlay)
├── rust-toolchain.toml             # Nightly toolchain pinning
└── hydra-mail/                     # Main project (v0.1.0)
    ├── src/
    │   ├── main.rs                 # CLI entry + daemon (592 LOC)
    │   ├── lib.rs                  # Module exports + tests (84 LOC)
    │   ├── config.rs               # Configuration + init (206 LOC)
    │   ├── channels.rs             # Pub/sub + replay buffer (257 LOC)
    │   ├── schema.rs               # Pulse message schema (115 LOC)
    │   └── toon.rs                 # TOON format support (56 LOC)
    ├── tests/
    │   └── integration_test.rs     # End-to-end workflow tests
    ├── docs/
    │   ├── ARCHITECTURE.md         # Current architecture (this file)
    │   └── SPEC.md                 # Full design spec (v0.1-v2 roadmap)
    ├── .claude-plugin/             # Plugin metadata for Claude Code
    ├── skills/hydra-mail/          # Claude Code skill definition
    └── .hydra/                     # Runtime (created by init)
        ├── config.toml             # Project UUID + socket path + topics
        ├── config.sh               # Shell env exports
        ├── hydra.sock              # Unix socket (created by daemon)
        ├── hydra-daemon            # Copied binary for reliability
        ├── daemon.pid              # Daemon process ID
        └── skills/
            └── hydra-mail.yaml     # Generated skill for Claude
```

### Source Metrics

| Component | LOC | Purpose |
|-----------|-----|---------|
| main.rs | 592 | CLI + daemon + connection handling |
| channels.rs | 257 | Broadcast channels + replay buffer |
| config.rs | 206 | Config parsing + skill generation |
| schema.rs | 115 | Pulse message struct + validation |
| lib.rs | 84 | Module exports + integration tests |
| toon.rs | 56 | TOON format enum + parsing |
| **Total** | **1,310** | Complete implementation |

## Core Components

### 1. CLI Binary (`hydra-mail`)

**Entry Point**: `src/main.rs`

**Commands**:

| Command | Purpose | Implementation |
|---------|---------|----------------|
| `init` | Initialize project | Creates `.hydra/`, generates config/skill YAML, optionally spawns daemon |
| `start` | Run daemon | Binds Unix socket, accepts connections, spawns async handlers |
| `emit` | Publish message | TOON encodes pulse → base64 → JSON command → Unix socket → daemon |
| `subscribe` | Listen to channel | Connects to daemon → receives replay buffer + live TOON strings |
| `status` | Show daemon info | Checks socket existence, reads PID, verifies process alive (no RPC) |
| `stop` | Stop daemon | Reads PID file → kills process → cleans up socket |

**CLI Framework**: `clap::Parser` with derive macros

**Async Runtime**: Tokio multi-threaded

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { .. } => { /* ... */ }
        Commands::Start { .. } => { /* ... */ }
        Commands::Emit { .. } => { /* ... */ }
        Commands::Subscribe { .. } => { /* ... */ }
        Commands::Status => { /* ... */ }
        Commands::Stop => { /* ... */ }
    }
}
```

### 2. Daemon Process

**Location**: `src/main.rs::handle_conn()` (lines 537-592)

**Responsibilities**:
- Listen on Unix Domain Socket (`.hydra/hydra.sock`)
- Accept incoming connections
- Parse JSON commands (`emit`, `subscribe`)
- Route messages to appropriate channels
- Handle per-connection async tasks via `tokio::spawn()`

**Lifecycle**:
```
init --daemon → Copy binary to .hydra/hydra-daemon
              → Spawn detached process
              → Write PID to daemon.pid
              → Redirect stdio to daemon.err

start         → Bind Unix socket
              → Accept connections in loop
              → Spawn async task per connection

stop          → Read daemon.pid
              → Kill process
              → Remove socket + PID file
```

**Connection Handling**:
```rust
async fn handle_conn(stream: UnixStream, config: Arc<Config>) -> anyhow::Result<()> {
    let (reader_side, mut writer) = stream.split();
    let mut reader = BufReader::new(reader_side).lines();

    while let Some(line) = reader.next_line().await? {
        let cmd: serde_json::Value = serde_json::from_str(&line)?;
        match cmd["cmd"].as_str() {
            Some("emit") => {
                // Decode base64, validate UTF-8, broadcast
                let data_b64 = cmd["data"].as_str()?;
                let data_bytes = base64::decode(data_b64)?;
                let toon_string = String::from_utf8(data_bytes)?;
                let receivers = emit_and_store(uuid, channel, toon_string)?;
                writer.write_all(response_json.as_bytes()).await?;
            }
            Some("subscribe") => {
                // Get history + live stream
                let (rx, history) = subscribe_broadcast(uuid, channel)?;
                for msg in history {
                    writer.write_all(msg.as_bytes()).await?;
                }
                while let Ok(msg) = rx.recv().await {
                    writer.write_all(msg.as_bytes()).await?;
                }
            }
            _ => return Err(anyhow!("Unknown command"))
        }
    }
}
```

### 3. Channel System

**Location**: `src/channels.rs` (257 LOC)

**Core Data Structure**:
```rust
static BROADCAST_CHANNELS: Lazy<Arc<Mutex<HashMap<
    (Uuid, String),  // Key: (project_uuid, topic_name)
    (broadcast::Sender<String>, ReplayBuffer)
>>>> = Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));
```

**Components**:

#### a. ReplayBuffer (Ring Buffer)
```rust
struct ReplayBuffer {
    messages: VecDeque<String>,  // TOON strings, not decoded
    capacity: usize,              // Default: 100
}

impl ReplayBuffer {
    fn push(&mut self, msg: String) {
        if self.messages.len() >= self.capacity {
            self.messages.pop_front();  // FIFO eviction
        }
        self.messages.push_back(msg);
    }

    fn messages(&self) -> Vec<String> {
        self.messages.iter().cloned().collect()
    }
}
```

**Purpose**: Provides history to late-joining subscribers

**Capacity**: 100 messages per channel (configurable but hardcoded)

**Storage Format**: TOON strings (not decoded on daemon side)

#### b. Broadcast Channel (Tokio)

**Type**: `tokio::sync::broadcast::channel(1024)`

**Capacity**: 1024 messages (overflow causes oldest message drop)

**Cloning**: Efficient via Arc wrapper, each subscriber gets independent receiver

**Scoping**: `(project_uuid, topic_name)` tuple ensures isolation

#### c. Channel Isolation Model

**Project Isolation**:
```rust
// Different UUIDs = separate namespaces
get_or_create_broadcast_tx(uuid1, "repo:delta")  // Channel A
get_or_create_broadcast_tx(uuid2, "repo:delta")  // Channel B (isolated)
```

**Topic Isolation**:
```rust
// Same UUID, different topics = separate channels
get_or_create_broadcast_tx(uuid, "repo:delta")    // Channel A
get_or_create_broadcast_tx(uuid, "team:alert")    // Channel B
```

**Concurrency Safety**: `Mutex` ensures atomic updates to channel map

#### d. Key Functions

**Get or Create Sender** (Lazy Initialization):
```rust
pub fn get_or_create_broadcast_tx(
    project_uuid: Uuid,
    topic: &str
) -> broadcast::Sender<String> {
    let mut channels = BROADCAST_CHANNELS.lock().unwrap();
    let key = (project_uuid, topic.to_string());

    channels.entry(key).or_insert_with(|| {
        let (tx, _rx) = broadcast::channel(1024);
        let replay = ReplayBuffer { messages: VecDeque::new(), capacity: 100 };
        (tx, replay)
    }).0.clone()
}
```

**Emit and Store** (Atomic Broadcast + Buffer):
```rust
pub fn emit_and_store(
    project_uuid: Uuid,
    topic: &str,
    message: String
) -> anyhow::Result<usize> {
    let mut channels = BROADCAST_CHANNELS.lock().unwrap();
    let key = (project_uuid, topic.to_string());

    let (tx, replay) = channels.get_mut(&key).ok_or_else(|| anyhow!("Channel not found"))?;

    replay.push(message.clone());  // Store in replay buffer
    let receivers = tx.receiver_count();
    tx.send(message)?;              // Broadcast to live subscribers

    Ok(receivers)
}
```

**Subscribe with History**:
```rust
pub fn subscribe_broadcast(
    project_uuid: Uuid,
    topic: &str
) -> anyhow::Result<(broadcast::Receiver<String>, Vec<String>)> {
    let channels = BROADCAST_CHANNELS.lock().unwrap();
    let key = (project_uuid, topic.to_string());

    let (tx, replay) = channels.get(&key).ok_or_else(|| anyhow!("Channel not found"))?;

    let history = replay.messages();  // Clone replay buffer
    let rx = tx.subscribe();          // Create new receiver

    Ok((rx, history))
}
```

### 4. Configuration System

**Location**: `src/config.rs` (206 LOC)

**Config Struct**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project_uuid: Uuid,           // UUID v4
    pub socket_path: PathBuf,         // Absolute path to .hydra/hydra.sock
    pub default_topics: Vec<String>,  // Pre-created channels
}
```

**Config File** (`.hydra/config.toml`):
```toml
[config]
project_uuid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
socket_path = "/absolute/path/to/.hydra/hydra.sock"
default_topics = ["repo:delta", "agent:presence"]
```

**Generated Files**:

#### a. Shell Integration (`config.sh`)
```bash
#!/bin/bash
export HYDRA_UUID="a1b2c3d4-e5f6-7890-abcd-ef1234567890"
export HYDRA_SOCKET="/absolute/path/to/.hydra/hydra.sock"
export HYDRA_FORMAT="toon"
```

**Purpose**: Allows shell scripts to source environment variables

#### b. Claude Code Skill (`skills/hydra-mail.yaml`)
```yaml
name: hydra-mail
description: Hydra Mail agent communication system
version: 0.1.0

tools:
  - name: hydra_emit
    description: Publish message to channel
    command: |
      hydra-mail emit \
        --project .hydra \
        --channel "$CHANNEL" \
        --type "$TYPE" \
        --data "$DATA" \
        --format toon

  - name: hydra_subscribe
    description: Subscribe to channel
    command: |
      hydra-mail subscribe \
        --project .hydra \
        --channel "$CHANNEL"
```

**Generation**: `Config::generate_skill_yaml()` creates this on `hydra init`

**Location**: `.hydra/skills/hydra-mail.yaml`

**Usage**: Manually upload to Claude Code session for agent integration

#### c. Directory Structure
```
.hydra/
├── config.toml          # Persistent config (TOML)
├── config.sh            # Env vars for shell scripts
├── hydra.sock           # Unix socket (created by daemon, mode 0600)
├── hydra-daemon         # Copied binary (mode 0700)
├── daemon.pid           # Process ID for management
├── daemon.err           # Daemon stderr log
└── skills/
    └── hydra-mail.yaml  # Claude Code skill
```

**Directory Permissions**: 0700 (rwx------)

**Socket Permissions**: 0600 (rw-------)

#### d. Key Functions

**Initialize Config**:
```rust
pub fn init(project_dir: &Path) -> anyhow::Result<Self> {
    let hydra_dir = project_dir.join(".hydra");
    fs::create_dir_all(&hydra_dir)?;
    fs::set_permissions(&hydra_dir, Permissions::from_mode(0o700))?;

    let socket_path = hydra_dir.canonicalize()?.join("hydra.sock");
    let config = Config {
        project_uuid: Uuid::new_v4(),
        socket_path,
        default_topics: vec!["repo:delta".into(), "agent:presence".into()],
    };

    // Write config.toml
    let toml_string = toml::to_string(&config)?;
    fs::write(hydra_dir.join("config.toml"), toml_string)?;

    // Generate config.sh
    config.generate_config_sh(&hydra_dir)?;

    // Generate skill YAML
    config.generate_skill_yaml(&hydra_dir)?;

    Ok(config)
}
```

**Load Config**:
```rust
pub fn load(project_dir: &Path) -> anyhow::Result<Self> {
    let config_path = project_dir.join(".hydra/config.toml");
    let toml_string = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&toml_string)?;
    Ok(config)
}
```

### 5. Message Schema

**Location**: `src/schema.rs` (115 LOC)

**Pulse Struct**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pulse {
    pub id: Uuid,                           // Unique ID (UUIDv4)
    pub timestamp: DateTime<Utc>,           // Created time (ISO 8601)
    pub pulse_type: String,                 // "delta", "status", "alert", etc.
    pub channel: String,                    // Topic name
    pub data: serde_json::Value,            // JSON payload (arbitrary)
    pub metadata: Option<serde_json::Value> // Optional metadata
}

impl Pulse {
    pub fn new(
        pulse_type: impl Into<String>,
        channel: impl Into<String>,
        data: serde_json::Value
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            pulse_type: pulse_type.into(),
            channel: channel.into(),
            data,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn validate_size(&self) -> anyhow::Result<()> {
        let json = serde_json::to_string(self)?;
        if json.len() > 10_000 {
            return Err(anyhow!("Pulse too large: {} bytes", json.len()));
        }
        Ok(())
    }
}
```

**JSON Pulse Example**:
```json
{
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "timestamp": "2025-11-26T12:34:56.789Z",
  "type": "delta",
  "channel": "repo:delta",
  "data": {
    "action": "file_modified",
    "path": "src/main.rs",
    "diff": "+10,-5"
  },
  "metadata": {
    "target": "agent_id_123"
  }
}
```

**Size Limit**: 10KB per pulse (enforced client-side before TOON encoding)

### 6. TOON Encoding

**Location**: `src/toon.rs` (56 LOC)

**Format Enum**:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageFormat {
    Toon,  // TOON only, no JSON fallback in v0.1.0
}

impl FromStr for MessageFormat {
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "toon" => Ok(MessageFormat::Toon),
            _ => Err(anyhow!("Unsupported format: {}", s)),
        }
    }
}

impl Default for MessageFormat {
    fn default() -> Self {
        MessageFormat::Toon
    }
}
```

**External Dependency**: `toon-format = "0.3"` crate

**Encoding Flow**:
```rust
// 1. Create JSON pulse
let pulse = Pulse::new("delta", "repo:delta", json!({"action": "test"}));
let pulse_json = serde_json::to_value(&pulse)?;

// 2. TOON encode with safe key folding
let encode_opts = EncodeOptions::new()
    .with_key_folding(KeyFoldingMode::Safe);
let toon_bytes = toon_format::encode(&pulse_json, &encode_opts)?;

// 3. Validate size
if toon_bytes.len() > 10_000 {
    return Err(anyhow!("Message too large"));
}

// 4. Base64 encode for JSON transport
let toon_string = String::from_utf8(toon_bytes)?;
let base64_data = base64::encode(toon_string.as_bytes());

// 5. Send via JSON command
let cmd = json!({
    "cmd": "emit",
    "channel": "repo:delta",
    "format": "toon",
    "data": base64_data
});
```

**Key Folding**: `KeyFoldingMode::Safe` optimizes repeated keys without overwriting duplicates

**Token Savings**: 30-60% vs JSON (empirical claim, needs quantification)

**Decoding**: Not implemented client-side in v0.1.0 (subscribers receive TOON strings)

## Data Flow

### Complete Message Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ Agent/User (Claude Code, CLI)                              │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       │ 1. Command: hydra-mail emit --channel repo:delta --data '...'
                       ▼
    ┌──────────────────────────────────────────┐
    │ CLI Binary (main.rs)                     │
    │ ┌──────────────────────────────────────┐ │
    │ │ Parse Arguments                      │ │
    │ │ Load config.toml                     │ │
    │ │ Extract: project_uuid, socket_path   │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Build Pulse JSON                     │ │
    │ │ {id, timestamp, type, channel, data} │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ TOON Encode                          │ │
    │ │ toon_format::encode(pulse_json)      │ │
    │ │ → TOON bytes                         │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Base64 Encode                        │ │
    │ │ base64::encode(toon_bytes)           │ │
    │ │ → Base64 string                      │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Build JSON Command                   │ │
    │ │ {"cmd":"emit","channel":"..."}       │ │
    │ │ {"data":"<base64>","format":"toon"}  │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Connect to Unix Socket               │ │
    │ │ UnixStream::connect(socket_path)     │ │
    │ └──────────────────────────────────────┘ │
    └──────────────┬───────────────────────────┘
                   │
                   │ 2. Send JSON line over Unix Domain Socket
                   │    {"cmd":"emit","channel":"repo:delta","data":"<base64>","format":"toon"}
                   ▼
    ┌──────────────────────────────────────────┐
    │ Daemon (handle_conn async task)          │
    │ ┌──────────────────────────────────────┐ │
    │ │ Read JSON Line                       │ │
    │ │ BufReader::read_line()               │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Parse JSON Command                   │ │
    │ │ serde_json::from_str()               │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Extract "data" Field (Base64)        │ │
    │ │ base64::decode()                     │ │
    │ │ → TOON bytes                         │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Validate UTF-8                       │ │
    │ │ String::from_utf8()                  │ │
    │ │ → TOON string                        │ │
    │ └──────────────────────────────────────┘ │
    └──────────────┬───────────────────────────┘
                   │
                   │ 3. Atomic operations in channels.rs
                   ├──────────────────────────────────┐
                   ▼                                  ▼
    ┌──────────────────────────────┐  ┌──────────────────────────────┐
    │ ReplayBuffer::push()         │  │ broadcast::Sender::send()    │
    │ - Store TOON string          │  │ - Broadcast to receivers     │
    │ - FIFO eviction at 100       │  │ - Non-blocking send          │
    │ - History for late join      │  │ - Each subscriber gets copy  │
    └──────────────────────────────┘  └──────────────────────────────┘
                   │                                  │
                   │ 4. Return JSON response          │
                   ▼                                  │
    ┌──────────────────────────────────────────┐     │
    │ Response JSON                            │     │
    │ {"status":"ok","receivers":5,"size":123} │     │
    │ → Write to Unix socket                   │     │
    └──────────────────────────────────────────┘     │
                                                       │
                   ┌───────────────────────────────────┘
                   │ 5. Subscribers receive
                   ▼
    ┌──────────────────────────────────────────┐
    │ Subscribers (live or replay)             │
    │ ┌──────────────────────────────────────┐ │
    │ │ Subscribe Command                    │ │
    │ │ {"cmd":"subscribe","channel":"..."}  │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Get History from ReplayBuffer        │ │
    │ │ subscribe_broadcast() → (rx, history)│ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Send History (100 messages max)      │ │
    │ │ for msg in history { write(msg) }    │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Stream Live Messages                 │ │
    │ │ while let Ok(msg) = rx.recv() {...}  │ │
    │ └──────────────────────────────────────┘ │
    │ ┌──────────────────────────────────────┐ │
    │ │ Print to stdout (TOON strings)       │ │
    │ │ println!("{}", msg);                 │ │
    │ └──────────────────────────────────────┘ │
    └──────────────────────────────────────────┘
```

### Flow Summary Matrix

| Flow | Initiator | Payload Format | Transport | Storage | Consumer |
|------|-----------|----------------|-----------|---------|----------|
| **Emit** | Agent/CLI | Pulse JSON → TOON → Base64 | Unix socket (JSON cmd) | ReplayBuffer (TOON string) | Live subscribers + late join |
| **Subscribe** | Agent/CLI | N/A | Unix socket (JSON cmd) | None | stdout (TOON strings) |
| **Init** | Agent/CLI | UUID + topics | TOML file | .hydra/ directory | Daemon + agents |
| **Status Check** | Agent/CLI | N/A | Process signals | PID file | Console output |
| **Stop** | Agent/CLI | N/A | Kill signal (SIGTERM) | None | Cleanup files |

## Message Protocol

### Command Protocol (JSON over Unix Socket)

**Emit Request**:
```json
{
  "cmd": "emit",
  "channel": "repo:delta",
  "format": "toon",
  "data": "<base64-encoded TOON string>"
}
```

**Emit Response**:
```json
{
  "status": "ok",
  "format": "toon",
  "size": 123,
  "receivers": 5
}
```

**Subscribe Request**:
```json
{
  "cmd": "subscribe",
  "channel": "repo:delta"
}
```

**Subscribe Response** (stream of TOON strings, newline-delimited):
```
<toon-encoded-message-1>\n
<toon-encoded-message-2>\n
<toon-encoded-message-3>\n
...
```

**Error Response**:
```json
{
  "status": "error",
  "msg": "Channel not found: invalid:topic"
}
```

### Protocol Invariants

1. **Line-Delimited JSON**: Each command/response is a single JSON line
2. **TOON-Only**: No JSON fallback in v0.1.0 (format field always "toon")
3. **Base64 Transport**: TOON bytes wrapped in base64 for JSON compatibility
4. **No Decode on Client**: Subscribers receive raw TOON strings (decode in future phase)
5. **Stateless Daemon**: No session state; each command is independent
6. **Replay-Then-Live**: Subscribe sends history before live messages

## CLI Commands

### Detailed Command Reference

#### 1. `init` - Initialize Project

**Purpose**: Create `.hydra/` directory, generate config, optionally spawn daemon

**Usage**:
```bash
hydra-mail init [--daemon]
```

**Options**:
- `--daemon`: Spawn persistent daemon after initialization

**Implementation** (main.rs:97-220):
```rust
Commands::Init { daemon } => {
    // 1. Create config
    let config = Config::init(&project_dir)?;

    // 2. Write config files
    config.save()?;
    config.generate_config_sh()?;
    config.generate_skill_yaml()?;

    // 3. Copy binary for reliability
    let exe = std::env::current_exe()?;
    let daemon_binary = project_dir.join(".hydra/hydra-daemon");
    fs::copy(&exe, &daemon_binary)?;
    fs::set_permissions(&daemon_binary, Permissions::from_mode(0o700))?;

    // 4. Optionally spawn daemon
    if daemon {
        Command::new(&daemon_binary)
            .arg("start")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(File::create(project_dir.join(".hydra/daemon.err"))?)
            .spawn()?;

        // Write PID
        let pid = child.id();
        fs::write(project_dir.join(".hydra/daemon.pid"), pid.to_string())?;
    }
}
```

**Output**:
```
Initialized Hydra in /path/to/project/.hydra
Project UUID: a1b2c3d4-e5f6-7890-abcd-ef1234567890
Socket: /path/to/project/.hydra/hydra.sock
Default topics: repo:delta, agent:presence
Daemon started with PID 12345
```

#### 2. `start` - Run Daemon

**Purpose**: Start daemon in foreground (or background via process management)

**Usage**:
```bash
hydra-mail start
```

**Implementation** (main.rs:222-252):
```rust
Commands::Start => {
    let config = Config::load(&project_dir)?;

    // Remove stale socket
    if config.socket_path.exists() {
        fs::remove_file(&config.socket_path)?;
    }

    // Bind Unix socket
    let listener = UnixListener::bind(&config.socket_path)?;
    fs::set_permissions(&config.socket_path, Permissions::from_mode(0o600))?;

    println!("Daemon listening on {:?}", config.socket_path);

    // Accept connections
    let config_arc = Arc::new(config);
    loop {
        let (stream, _) = listener.accept().await?;
        let config_clone = Arc::clone(&config_arc);
        tokio::spawn(async move {
            if let Err(e) = handle_conn(stream, config_clone).await {
                eprintln!("Connection error: {}", e);
            }
        });
    }
}
```

**Output**:
```
Daemon listening on "/path/to/project/.hydra/hydra.sock"
```

#### 3. `emit` - Publish Message

**Purpose**: Encode pulse to TOON, send to daemon for broadcasting

**Usage**:
```bash
hydra-mail emit \
  --project .hydra \
  --channel repo:delta \
  --type delta \
  --data '{"action":"file_modified","path":"src/main.rs"}' \
  --format toon
```

**Options**:
- `--project <path>`: Path to `.hydra` directory
- `--channel <topic>`: Channel name (e.g., `repo:delta`, `team:alert`)
- `--type <type>`: Pulse type (e.g., `delta`, `status`, `alert`)
- `--data <json>`: JSON payload (or `@-` to read from stdin)
- `--format <format>`: Message format (only `toon` in v0.1.0)
- `--target <id>`: Optional target agent ID (stored in metadata)

**Implementation** (main.rs:254-373):
```rust
Commands::Emit { channel, r#type, data, format, target, .. } => {
    let config = Config::load(&project_dir)?;

    // 1. Parse data
    let data_json: serde_json::Value = if data == "@-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        serde_json::from_str(&buf)?
    } else {
        serde_json::from_str(&data)?
    };

    // 2. Build pulse
    let pulse_json = json!({
        "id": Uuid::new_v4(),
        "timestamp": Utc::now(),
        "type": r#type,
        "channel": channel,
        "data": data_json,
        "metadata": target.map(|t| json!({"target": t}))
    });

    // 3. TOON encode
    let encode_opts = EncodeOptions::new().with_key_folding(KeyFoldingMode::Safe);
    let toon_bytes = toon_format::encode(&pulse_json, &encode_opts)?;

    // 4. Validate size
    if toon_bytes.len() > 10_000 {
        return Err(anyhow!("Message too large: {} bytes", toon_bytes.len()));
    }

    // 5. Base64 encode
    let toon_string = String::from_utf8(toon_bytes)?;
    let base64_data = base64::encode(toon_string.as_bytes());

    // 6. Build command
    let cmd = json!({
        "cmd": "emit",
        "channel": channel,
        "format": "toon",
        "data": base64_data
    });

    // 7. Send to daemon
    let mut stream = UnixStream::connect(&config.socket_path).await?;
    stream.write_all(format!("{}\n", cmd).as_bytes()).await?;

    // 8. Read response
    let mut reader = BufReader::new(&mut stream).lines();
    let response = reader.next_line().await?.ok_or_else(|| anyhow!("No response"))?;
    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["status"] == "ok" {
        println!("Emitted to {} ({} receivers, {} bytes)",
                 channel, resp["receivers"], resp["size"]);
    } else {
        eprintln!("Error: {}", resp["msg"]);
    }
}
```

**Output**:
```
Emitted to repo:delta (5 receivers, 123 bytes)
```

#### 4. `subscribe` - Listen to Channel

**Purpose**: Connect to daemon, receive replay buffer + live messages

**Usage**:
```bash
hydra-mail subscribe --project .hydra --channel repo:delta
```

**Options**:
- `--project <path>`: Path to `.hydra` directory
- `--channel <topic>`: Channel name to subscribe to
- `--once`: Exit after receiving first message (for testing)

**Implementation** (main.rs:375-425):
```rust
Commands::Subscribe { channel, once, .. } => {
    let config = Config::load(&project_dir)?;

    // 1. Build subscribe command
    let cmd = json!({
        "cmd": "subscribe",
        "channel": channel
    });

    // 2. Connect to daemon
    let mut stream = UnixStream::connect(&config.socket_path).await?;
    stream.write_all(format!("{}\n", cmd).as_bytes()).await?;

    // 3. Stream messages
    let (reader_side, _writer) = stream.split();
    let mut reader = BufReader::new(reader_side).lines();

    println!("Subscribed to {}", channel);

    while let Some(line) = reader.next_line().await? {
        println!("{}", line);  // Print TOON string

        if once {
            break;
        }
    }
}
```

**Output**:
```
Subscribed to repo:delta
<toon-encoded-message-1>
<toon-encoded-message-2>
...
```

#### 5. `status` - Show Daemon Status

**Purpose**: Check if daemon is running, show project info (no RPC to daemon)

**Usage**:
```bash
hydra-mail status
```

**Implementation** (main.rs:427-490):
```rust
Commands::Status => {
    let config = Config::load(&project_dir)?;

    println!("Project UUID: {}", config.project_uuid);
    println!("Socket: {:?}", config.socket_path);
    println!("Default topics: {:?}", config.default_topics);

    // Check socket existence
    if config.socket_path.exists() {
        println!("Socket exists: yes");
    } else {
        println!("Socket exists: no (daemon not running)");
    }

    // Check PID file
    let pid_path = project_dir.join(".hydra/daemon.pid");
    if pid_path.exists() {
        let pid_str = fs::read_to_string(&pid_path)?;
        let pid: u32 = pid_str.trim().parse()?;

        // Check if process is alive (Unix-specific)
        let alive = Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .output()?
            .status
            .success();

        if alive {
            println!("Daemon: running (PID {})", pid);
        } else {
            println!("Daemon: stale PID {} (process not found)", pid);
        }
    } else {
        println!("Daemon: not running (no PID file)");
    }
}
```

**Output**:
```
Project UUID: a1b2c3d4-e5f6-7890-abcd-ef1234567890
Socket: "/path/to/project/.hydra/hydra.sock"
Default topics: ["repo:delta", "agent:presence"]
Socket exists: yes
Daemon: running (PID 12345)
```

#### 6. `stop` - Stop Daemon

**Purpose**: Kill daemon process, clean up socket and PID file

**Usage**:
```bash
hydra-mail stop
```

**Implementation** (main.rs:492-531):
```rust
Commands::Stop => {
    let config = Config::load(&project_dir)?;
    let pid_path = project_dir.join(".hydra/daemon.pid");

    if !pid_path.exists() {
        eprintln!("No PID file found (daemon not running)");
        return Ok(());
    }

    // Read PID
    let pid_str = fs::read_to_string(&pid_path)?;
    let pid: u32 = pid_str.trim().parse()?;

    // Kill process
    Command::new("kill")
        .arg(pid.to_string())
        .status()?;

    // Wait for process to exit
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Cleanup
    fs::remove_file(&pid_path)?;
    if config.socket_path.exists() {
        fs::remove_file(&config.socket_path)?;
    }

    println!("Daemon stopped (PID {})", pid);
}
```

**Output**:
```
Daemon stopped (PID 12345)
```

## Testing

### Test Categories

#### 1. Unit Tests (in source modules)

**Location**: `src/lib.rs`, `src/config.rs`, `src/channels.rs`, `src/schema.rs`, `src/toon.rs`

**Coverage**:
- Config serialization/deserialization
- Channel isolation (project + topic scoping)
- Replay buffer FIFO behavior
- Pulse validation and size limits
- TOON format parsing

**Example** (channels.rs):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_replay_buffer_capacity_limit() {
        let uuid = Uuid::new_v4();
        let topic = "test:limit";

        // Emit 150 messages
        for i in 0..150 {
            emit_and_store(uuid, topic, format!("msg-{}", i)).unwrap();
        }

        // Subscribe and check history
        let (_rx, history) = subscribe_broadcast(uuid, topic).unwrap();

        // Should only have last 100 messages
        assert_eq!(history.len(), 100);
        assert_eq!(history[0], "msg-50");   // First in buffer
        assert_eq!(history[99], "msg-149"); // Last in buffer
    }
}
```

#### 2. Integration Tests

**Location**: `tests/integration_test.rs`

**Tests**:
- `test_init_creates_hydra()`: Verifies `.hydra/` creation and file contents
- `test_emit_subscribe_end_to_end()`: Full workflow with daemon

**Example**:
```rust
#[tokio::test]
async fn test_emit_subscribe_end_to_end() {
    let temp = tempfile::tempdir().unwrap();
    let project_dir = temp.path();

    // 1. Init
    Command::new(BINARY_PATH)
        .arg("init")
        .current_dir(project_dir)
        .status()
        .unwrap();

    // 2. Start daemon
    let mut daemon = Command::new(BINARY_PATH)
        .arg("start")
        .current_dir(project_dir)
        .spawn()
        .unwrap();

    std::thread::sleep(Duration::from_secs(1));

    // 3. Subscribe in background
    let mut subscriber = Command::new(BINARY_PATH)
        .arg("subscribe")
        .arg("--channel").arg("test:topic")
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    std::thread::sleep(Duration::from_millis(500));

    // 4. Emit message
    Command::new(BINARY_PATH)
        .arg("emit")
        .arg("--channel").arg("test:topic")
        .arg("--type").arg("test")
        .arg("--data").arg(r#"{"msg":"hello"}"#)
        .current_dir(project_dir)
        .status()
        .unwrap();

    // 5. Check subscriber received message
    let output = subscriber.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("msg"));

    // Cleanup
    daemon.kill().unwrap();
}
```

### Test Execution

```bash
# Run all tests
cargo test

# Run with output (for debugging)
cargo test -- --nocapture

# Run specific test
cargo test test_replay_buffer_capacity_limit

# Run integration tests only
cargo test integration_test

# Run in release mode (performance)
cargo test --release
```

## Performance Characteristics

### Claimed Metrics

| Metric | Value | Status |
|--------|-------|--------|
| Latency | <5ms | Claimed (not verified) |
| Throughput | 1M+ events/sec | Claimed (not verified) |
| Token Savings | 30-60% vs JSON | Claimed (not quantified) |
| Binary Size | ~1 MB | Approximate |
| Message Size Limit | 10 KB | Enforced |
| Replay Buffer Size | 100 messages | Hardcoded |
| Broadcast Capacity | 1024 messages | Hardcoded |
| Memory per Channel | Minimal | Not measured |

### Performance Considerations

**Fast Paths**:
- Unix Domain Sockets (no TCP overhead)
- Tokio broadcast channels (zero-copy for multiple receivers)
- In-memory replay buffer (no disk I/O)
- TOON encoding (smaller payloads than JSON)

**Bottlenecks**:
- Base64 encoding/decoding (CPU overhead)
- Mutex lock contention on channel map (single global lock)
- ReplayBuffer cloning on subscribe (copies all 100 messages)
- TOON encoding overhead (not yet measured)

**Scalability Limits**:
- Single process (no distributed coordination)
- Global mutex (limits concurrent emits/subscribes)
- Fixed replay buffer size (100 messages per channel)
- No message batching (one socket write per message)

## Known Gaps

### Missing vs. Spec (docs/SPEC.md)

1. **MPSC Channels**: No targeted/point-to-point messaging (Phase 2)
2. **Mode System**: No inject/loop/hybrid modes (Phase 2)
3. **SDK Injection**: No automatic agent instrumentation (Phase 2)
4. **Sled Durability**: Optional feature not tested or documented (Phase 2)
5. **Client TOON Decoding**: Subscribers receive raw TOON strings (Phase 2)
6. **JSON Fallback**: No compatibility mode for non-TOON agents (Phase 3)
7. **Windows Support**: No named pipes implementation (Phase 3)
8. **Network Transport**: TCP/TLS for cross-host not planned (Future)

### Documentation Gaps

1. **No Executable Examples**: No example agents or usage demos in repository
2. **Missing Benchmarks**: Performance claims not backed by benchmark suite
3. **No Error Code Reference**: Generic error messages, no troubleshooting guide
4. **Limited Recovery Docs**: No daemon crash recovery procedures
5. **Stale PID Handling**: No automatic cleanup of stale PID files

### Testing Gaps

1. **No Concurrency Tests**: No tests for concurrent emits/subscribes
2. **No Malformed Input Tests**: No tests for invalid JSON commands
3. **No Permission Tests**: No tests for socket permission errors
4. **Integration Tests Slow**: Require release binary build (slow CI)

### Implementation Issues

1. **Skill YAML Hardcoded**: `config.rs` contains YAML template (should be external)
2. **Daemon Lifecycle Fragile**: PID file can become stale if daemon crashes
3. **No Daemon Health Check**: `status` inspects files, not RPC to daemon
4. **Global Mutex Bottleneck**: Single lock for all channels (limits concurrency)
5. **No Message Batching**: One socket write per message (inefficient for bursts)

### Security Considerations

**Current Protections**:
- Unix socket permissions (0600, owner-only)
- Directory permissions (0700, owner-only)
- Project UUID isolation (prevents cross-project interference)
- Message size limits (10KB max, prevents abuse)
- No network exposure (local-only by design)

**Missing Protections**:
- No HMAC/authentication (planned for Phase 3)
- No message integrity checks (relies on Unix socket security)
- No rate limiting (allows flood attacks)
- No input sanitization (trusts well-formed JSON)

## Appendix: File Locations

### Source Files

| File | LOC | Purpose |
|------|-----|---------|
| src/main.rs | 592 | CLI entry + daemon + connection handling |
| src/channels.rs | 257 | Broadcast channels + replay buffer |
| src/config.rs | 206 | Config parsing + skill generation |
| src/schema.rs | 115 | Pulse message struct + validation |
| src/lib.rs | 84 | Module exports + integration tests |
| src/toon.rs | 56 | TOON format enum + parsing |

### Test Files

| File | Purpose |
|------|---------|
| tests/integration_test.rs | End-to-end workflow tests |
| src/lib.rs (tests) | Config/channel integration tests |
| src/config.rs (tests) | Config serialization tests |
| src/channels.rs (tests) | Channel behavior tests |
| src/schema.rs (tests) | Pulse validation tests |
| src/toon.rs (tests) | Format parsing tests |

### Documentation Files

| File | Purpose |
|------|---------|
| README.md (root) | Monorepo overview |
| README.md (hydra-mail) | Project overview + quick start |
| INSTALLATION.md | Installation guide |
| CLAUDE.md | Claude Code guidance |
| docs/ARCHITECTURE.md | Current architecture (this file) |
| docs/SPEC.md | Full design spec (v0.1-v2 roadmap) |
| .claude-plugin/README.md | Plugin usage guide |
| skills/hydra-mail/SKILL.md | Skill reference |

### Build Configuration

| File | Purpose |
|------|---------|
| Cargo.toml | Rust package manifest |
| flake.nix | Nix build configuration |
| rust-toolchain.toml | Rust toolchain pinning (nightly) |
| .gitignore | Git ignore patterns |

### Runtime Files (created by `init`)

| File | Purpose |
|------|---------|
| .hydra/config.toml | Project config (UUID, socket, topics) |
| .hydra/config.sh | Shell env exports |
| .hydra/hydra.sock | Unix socket (created by daemon) |
| .hydra/hydra-daemon | Copied binary for reliability |
| .hydra/daemon.pid | Daemon process ID |
| .hydra/daemon.err | Daemon stderr log |
| .hydra/skills/hydra-mail.yaml | Generated Claude Code skill |

---

**Document Version**: v0.1.0
**Last Updated**: 2025-11-26
**Authors**: Hydra Tools Contributors
