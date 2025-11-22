# Hydra Mail: Tokio Channels-Based Architecture Design Document

## Document Metadata
- **Title**: Hydra Mail - Lightweight In-Memory Pub/Sub Protocol for Local Agent State Broadcasting (Tokio Channels Implementation)
- **Version**: 0.1.0 (Skills-First MVP + Phased Integration)
- **Date**: November 14, 2025
- **Author**: Grok (xAI Assistant) - Adapted from prior Rust spec, incorporating Tokio channels and project-aware init
- **Status**: Ready for Engineering Handover (Phased Implementation)
- **Audience**: Lead Engineer / Rust Development Team
- **Dependencies**: Rust 1.91.1 (stable), Cargo, Tokio 1.40.0+ (minimal features)
- **Estimated Effort**: Phase 1: 1-2 days (Skills YAML gen); Phase 2: 1 day (modes); Phase 3: 1-2 days (SDK/TOON); Total: 3-5 days
- **Rationale for Tokio Channels**: Based on 2025 ecosystem research (Rust 1.91.1 stable; Tokio 1.40 updates for async ergonomics), Tokio's built-in `broadcast` and `mpsc` channels provide zero-dependency, in-memory pub/sub ideal for local (same-machine) agent communication. Sub-μs latency, thread-safe, and scales to 10-20 subscribers without external brokers like Redis—aligning with local-only needs (no network/distributed).
- **Rationale for Skills-Centric UX**: Skills provide prompt-native, extensible integration that "teaches" agents Hydra usage without always-on overhead. Generated YAML on init enables upload-once workflow, with hooks as opt-in enhancements for advanced modes.

## 1. Executive Summary
This iteration refines Hydra Mail to a pure in-memory, Tokio channels-driven architecture, eliminating external deps (e.g., Redis) for ultimate lightness (~1MB binary). Agents (e.g., Claude Code via Skill, Codex CLI wrappers) broadcast "pulses" (TOON-encoded state deltas) via `tokio::sync::broadcast` for fan-out, with `mpsc` for point-to-point handoffs (e.g., polls/acks). To enable seamless multi-agent collaboration, Hydra now includes project-specific initialization: `hydra init` creates a `.hydra` directory with configuration (e.g., project ID, default topics), spawning an optional persistent daemon for shared channels. This supports collaborative coding swarms on shared codebases, reducing coordination latency to <5ms while keeping token use <200/session via Skills-based integration.

**Key Benefits**:
- **Local-Optimized**: In-memory only—no files/pipes/IO for core pub/sub; optional sled for persistence.
- **Project-Aware**: `init` scaffolds per-project state; auto-detection for agents.
- **Skills-First**: Init generates mode-agnostic YAML for Claude; Agents auto-detect .hydra via Skill instructions; Upload once for prompt-driven integration.
- **Dual Modes** (Phase 2): Config.toml sets inject (push), loop (hooks), or hybrid (urgent push + routine pull); Skills adapt via embedded instructions.
- **Advanced Layering** (Phase 3): SDK integration for inject mode (AgentState fusion); Full TOON for 30-60% token savings.
- **Performant**: Tokio's zero-copy channels enable 1M+ events/sec in benchmarks; async non-blocking.
- **Lightweight**: Single binary; Tokio features=["sync"] (~300KB add); no runtime overhead.
- **Token-Efficient**: TOON format reduces message payload by 30-60% vs JSON, cutting LLM context usage.
- **Simple**: CLI commands spawn channels; Generated Skills teach usage; Hooks opt-in for advanced flows.

MVP Scope (Phase 1): Tokio broadcast as default broker; Auto-generated YAML Skills for Claude; Simple emit/subscribe via prompts. Future: Mode layering (Phase 2), SDK inject (Phase 3), sled for durable queues.

## 2. Goals and Non-Goals
### Goals
- Implement thread/process-safe pub/sub for 5-20 local agents, with topics via channel sharding.
- Achieve <5ms end-to-end latency for emits in 10-agent swarms; <1MB RAM for 10k pulses.
- Ensure zero external deps for core (Tokio std); cross-platform binaries.
- Enable project integration: `hydra init` for scaffolding; auto-registration for agents like Claude/Codex.
- Validate: E2E tests with mock agents; measure 30-60% token savings with TOON vs JSON in Claude loops.

### Non-Goals (v1)
- Persistence by default (in-memory ephemeral)—add sled as opt-in feature.
- Advanced topics (e.g., regex matching)—simple string prefixes (e.g., "repo:stock.delta").
- Distributed scaling (e.g., IPC over net)—local threads/processes only.
- GUI/SSE—stdout streams for subscribers; v1.1 via axum if needed.

## 3. System Overview
Hydra Mail uses Tokio channels for a pure in-memory broker: `broadcast` for one-to-many pulses (e.g., delta broadcasts to all coders), `mpsc` for targeted responses (e.g., ack polls). The Rust CLI manages channel lifecycles (spawn on init, cleanup on exit), invoked by Claude Skill tools. For project integration, `hydra init` creates a `.hydra` directory in the project root, generating a config.toml (with project_uuid, default_topics) and optionally spawning a daemon process (via `--daemon` flag) that runs persistently, binding channels to the project scope via the UUID or path. Agents launched in the project (e.g., Claude Code session or Codex CLI) detect the `.hydra` dir, read the config, and auto-register by invoking the binary with project-specific flags (e.g., `--project .hydra`). Inter-process RPC between the CLI and the daemon uses a Unix Domain Socket (UDS) path stored in `.hydra/config.toml`, targeting Linux and macOS for v1 (Windows out-of-scope). This leverages Tokio's 2025 updates (1.40: improved `select!` macros for multi-channel muxing) for seamless async integration without IO.

### High-Level Diagram
```
[Project Root (e.g., /path/to/my-project)]
├── .hydra/
│   ├── config.toml   # project_uuid, default_topics, socket, mode (Phase 2)
│   ├── config.sh     # export HYDRA_UUID=...; export HYDRA_SOCKET=... (Phase 1)
│   ├── skills/
│   │   └── hydra-mail.yaml  # Generated YAML for Claude (Phase 1)
│   └── daemon.pid    # If daemon mode
├── hydra-mail binary (in PATH or local)
└── Agents (Claude/Codex)

[Claude Code Session (Publisher) - Phase 1]
  ├── Skill (Auto-Generated): Detect .hydra → Source config.sh → Prompt-driven emit/read
  └── Tool: shell("source .hydra/config.sh && hydra-mail emit --project .hydra --type delta --data @- --channel 'repo:stock'")
          ↓ (Exec Binary, Scoped to Project)
[Rust CLI Binary (In-Memory Broker, Project-Scoped)]
  ├── src/main.rs: clap args → tokio::spawn(broker_task); Load .hydra/config
  ├── src/channels.rs: Static HashMap<String, (Sender, Receiver)> keyed by project_uuid + topic
  │   ├── Broadcast: tokio::sync::broadcast::channel::<String>(1024)  // Fan-out pulses
  │   └── MPSC: tokio::sync::mpsc::channel::<String>(1024)  // Point-to-point acks
  ├── src/modes.rs (Phase 2): Traits for InjectMode, LoopMode, HybridMode
  │   └── [Daemon] → Mode Router → Inject Buffer | Loop Hooks | Hybrid Route
  ├── src/schema.rs: serde for Pulse validation
  └── src/toon.rs: TOON encoding/decoding for token-efficient payloads
          ↓ (Async Non-Blocking, Daemon if --daemon)
[Subscriber Agents (e.g., Codex Wrapper) - Phase 1]
  ├── Auto-Detect (Skills): If .hydra exists, source config.sh → subscribe via alias
  └── Tokio Loop: select! { msg = rx.recv() => handle(msg) }
[Phase 2: Mode-Specific Flows]
  ├── InjectMode: Daemon spawns listener → Urgent pulses → AgentState inject
  ├── LoopMode: Pre-tool hooks → hydra_read --since $HYDRA_TS → Summarize
  └── HybridMode: Urgent (inject) + Routine (loop read)
[Phase 3: Advanced]
  └── SDK Integration: anthropic_sdk::AgentState.inject(decode_toon(pulse))
[Optional: Sled Queue] → Feature: Append for durability (v1.1)
```

- **Data Flow**:
  1. **Init**: `hydra init` in project root → Create .hydra/dir, config.toml (UUID, defaults), optional --daemon to spawn persistent broker.
  1.5. **Skills Generation** (Phase 1): Init generates `.hydra/skills/hydra-mail.yaml`—upload to Claude for prompt-driven integration. Also creates `.hydra/config.sh` for env exports (UUID, socket path).
  2. **Emit**: Agent detects .hydra via Skill instructions → Skill tools source config.sh → CLI connects to daemon via Unix socket and proxies `emit`; daemon TOON-encodes payload and executes `tx.send(toon_data).await?;` (broadcast for pulses, mpsc for targeted; scoped to project_uuid).
  3. **Subscribe/Auto-Register**: Agent checks for .hydra (via Skills) → CLI connects to daemon via Unix socket and requests a subscription; Return receiver stream; async loop prints/callbacks (e.g., spawn reactor task).
  4. **Mode Routing** (Phase 2): Daemon loads config.toml mode → Routes to InjectMode (listener spawn), LoopMode (read/write hooks), or HybridMode (urgent filter).
  5. **Cleanup**: Drop on binary exit; daemon persists until killed (e.g., `hydra stop`); optional sled flush.
  6. **Resilience**: Channel bounds (1024) prevent OOM; `Result` for send errors.

### Key Assumptions
- Agents spawn via CLI wrappers or Skills that check for .hydra in cwd and invoke with --project flag.
- Tokio runtime per binary instance; multi-instance via OS processes, but shared via project daemon (single process per project).
- Pulses as TOON-encoded strings (30-60% smaller than JSON); serde for parse, TOON for token-efficient transport.
- Project scoping via UUID in config.toml to isolate channels (e.g., key = format!("{project_uuid}:{topic}")).
- v1 targets Linux and macOS using Unix Domain Sockets; Windows support deferred.

## 4. Core Components
### 4.1 Claude Skill (Generated YAML: ~2KB, Mode-Aware)
**Phase 1 MVP**: `hydra init` auto-generates `.hydra/skills/hydra-mail.yaml` from config.toml variables (UUID, socket path, default topics). Upload once to Claude for prompt-driven integration.

**YAML Structure** (Tiered Instructions + Tools):
- **Tier 1 (High Priority: Core Instructions, ~100 tokens)**:
  - "Hydra Mail enables multi-agent collaboration via pub/sub channels. Detect `.hydra` in cwd. After completing actions (e.g., file edits, planning), emit deltas to share state with other agents. Use tools below—they auto-detect config."

- **Tier 2 (Medium Priority: Tools + Schema, ~150 tokens)**:
  - **Tool: `hydra_emit`**: Publish a message to the swarm
    ```yaml
    - name: hydra_emit
      description: Emit a pulse to share state with other agents
      parameters:
        - name: channel
          type: string
          description: Topic (e.g., "repo:delta", "team:alert")
        - name: type
          type: string
          description: Pulse type (e.g., "delta", "status", "question")
        - name: data
          type: string
          description: JSON data payload
      command: |
        if [ -d ".hydra" ]; then
          source .hydra/config.sh
          printf '%s\n' "$data" | hydra-mail emit --project .hydra --channel "$channel" --type "$type" --data @-
        else
          echo "Hydra not initialized. Run: hydra-mail init --daemon" >&2
          exit 1
        fi
    ```

  - **Tool: `hydra_subscribe`** (Phase 1): Listen to messages (for debugging/testing)
    ```yaml
    - name: hydra_subscribe
      description: Subscribe to a channel to see messages from other agents
      parameters:
        - name: channel
          type: string
        - name: once
          type: boolean
          default: true
      command: |
        if [ -d ".hydra" ]; then
          source .hydra/config.sh
          if [ "$once" = "true" ]; then
            hydra-mail subscribe --project .hydra --channel "$channel" --once
          else
            hydra-mail subscribe --project .hydra --channel "$channel"
          fi
        else
          echo "Hydra not initialized" >&2
          exit 1
        fi
    ```

- **Tier 3 (Low Priority: Examples, ~100 tokens)**:
  - "Example: After editing routes.py, emit: `hydra_emit channel='repo:delta' type='delta' data='{\"file\":\"routes.py\",\"action\":\"updated\"}'`"
  - "Check for messages: `hydra_subscribe channel='repo:delta' once=true`"

**Phase 2 Extensions** (Mode-Aware):
- Add `hydra_read` tool with `--since $HYDRA_TS` for loop mode
- Update Tier 1 instructions: "If config mode=loop, read pre-tool; If mode=inject, auto-receive via listener"
- Embed mode detection in tools: `source .hydra/config.sh; if [ "$HYDRA_MODE" = "loop" ]; then ...`

**Data Input**: Tools use stdin (`--data @-`) to avoid shell escaping issues with JSON.

**Efficiency**: Skills trigger at agent cadence (~2-5x/session); Binary handles all I/O; Clear exit codes (0=success, 1=error) with actionable stderr.

### 4.2 Rust CLI Binary (Core: ~300 LoC + Phase 2 modes)
- **Role**: Channel manager—async tasks for pub/sub, with clap for commands; project-aware via .hydra; mode-aware routing.
- **Crate Structure**:
  ```
  src/
  ├── main.rs          # #[tokio::main] entry; clap::Parser; Load .hydra/config
  ├── schema.rs        # #[derive(serde::Deserialize)] pub struct Pulse { ... }
  ├── channels.rs      // Core: once_cell::sync::Lazy<HashMap<String, BroadcastChannel>>; Scoped by project
  ├── config.rs        // Parse .hydra/config.toml; Generate UUID; Generate Skills YAML + config.sh (Phase 1)
  ├── modes.rs         // Phase 2: Traits for InjectMode, LoopMode, HybridMode routing
  ├── toon.rs          // TOON encoding/decoding for token-efficient payloads
  └── lib.rs           // Exports + tests
  Cargo.toml           // Tokio + minimal deps + toml crate
  ```
- **Phase 1 Additions** (Skills Generation):
  - `config.rs`: Add `generate_skill_yaml()` to template `.hydra/skills/hydra-mail.yaml` from config vars
  - `config.rs`: Add `generate_config_sh()` to export env vars (HYDRA_UUID, HYDRA_SOCKET)
  - `init` command: Call both generators after creating config.toml
- **Phase 2 Additions** (Mode Support):
  - `modes.rs`: Define traits `InjectMode`, `LoopMode`, `HybridMode` with `handle_pulse()` methods
  - `config.rs`: Add `mode` field to Config struct (default: "hybrid")
  - Daemon: Load mode from config; route messages via trait dispatch
- **Key Crates** (Ultra-Light, 2025 Ecosystem):
  - `clap = { version = "4.5", features = ["derive"] }` : CLI parsing.
  - `serde = { version = "1.0", features = ["derive"] }` + `serde_json` : Pulse JSON.
  - `tokio = { version = "1.40", features = ["sync", "rt-multi-thread"] }` : Channels + runtime (broadcast/mpsc; ~300KB).
  - `once_cell = "1.19"` : Lazy global channels (thread-safe init).
  - `uuid = { version = "1.10", features = ["v4"] }` : Pulse IDs + project UUID.
  - `toml = "0.8"` : Parse config.toml.
  - Optional Feature: `sled = "0.34"` for durable mode.
- **Commands** (clap::Subcommand):
  - `init`: In project root → Create .hydra/ dir, config.toml (UUID, defaults), optional --daemon to spawn persistent broker.
  - `start`/`daemon`: `--project <PATH>` → Spawn persistent runtime; Listen for subcommands via Unix Domain Socket (local only).
  - `emit`: `let channel = get_or_create_tx(project_uuid, topic); channel.send(json).await?;` (broadcast default; --target for mpsc). Supports `--data @-` for stdin JSON input.
  - `subscribe`: `let rx = get_rx(project_uuid, topic); tokio::spawn(async move { while let Ok(msg) = rx.recv().await { println!("{}", msg); } });` — Streams to stdout; Auto if --project provided. Options: `--format lines` (line-delimited JSON), `--callback <SCRIPT>` (pipe to script), `--once` (single message then exit).
  - `status`: Show daemon PID, socket path, topics, subscribers (quick stats).
  - `stop`: Kill daemon if running (read .hydra/daemon.pid).
- **Channel Manager** (in channels.rs, project-scoped):
  ```rust
  use std::collections::HashMap;
  use std::sync::Arc;
  use once_cell::sync::Lazy;
  use tokio::sync::{broadcast, mpsc};
  use uuid::Uuid;

  static CHANNELS: Lazy<Arc<tokio::sync::Mutex<HashMap<(Uuid, String), broadcast::Sender<String>>>>> =
      Lazy::new(|| Arc::new(tokio::sync::Mutex::new(HashMap::new())));

  pub async fn get_or_create_tx(project_uuid: Uuid, topic: &str) -> broadcast::Sender<String> {
      let key = (project_uuid, topic.to_string());
      let mut map = CHANNELS.lock().await;
      map.entry(key)
          .or_insert_with(|| broadcast::channel(1024).0)
          .clone()
  }

  pub async fn subscribe(project_uuid: Uuid, topic: &str) -> broadcast::Receiver<String> {
      let tx = get_or_create_tx(project_uuid, topic).await;
      tx.subscribe()
  }
  ```
  - Broadcast for fan-out; mpsc variant for polls (separate HashMap, keyed similarly).
  - In daemon mode: Persistent loop handles all invokes, sharing channels in single process.

### 4.3 Pulse Schema (Rust Model, Enhanced for TOON)
- Serde-derived as prior; add `#[serde(rename_all = "snake_case")]` for JSON compatibility.
- TOON Support: Implement `ToonEncode` and `ToonDecode` traits for automatic TOON serialization.
- Validation: Custom deserializer for size (<1KB TOON payload); reject on emit to prevent channel bloat.
- Default Encoding: Messages stored internally as TOON for 30-60% size reduction; JSON fallback for compatibility.

### 4.4 Agent Integration (Simple Wrappers and Hooks)
- **Generic POSIX Wrapper** (for any shell-capable agent; save as `hydra-emit.sh`, chmod +x):
  ```bash
  #!/bin/bash
  set -euo pipefail
  channel="${1:?channel}"; type="${2:?type}"
  if [ -d ".hydra" ]; then
    cat | hydra-mail emit --project .hydra --channel "$channel" --type "$type" --data @-
  else
    echo "Hydra not initialized. Run: hydra init --daemon" >&2
    exit 1
  fi
  ```
  Usage in Skills/hooks: `printf '%s\n' '{"file":"routes.py"}' | ./hydra-emit.sh repo:delta delta`. Similar for subscribe: `hydra-mail subscribe --project .hydra --channel repo:delta --format lines --callback ./handle-pulse.sh`.

## 5. Data Flow and Interfaces
- **CLI Args** (clap):
  - Init: `[--daemon]` to spawn persistent broker; `[--mode <inject|loop|hybrid>]` for Phase 2.
  - Emit: `--project <PATH> --type <PulseType> --data <JSON or @-> --channel <STR> [--target <AGENT_ID> for mpsc]`.
  - Subscribe: `--project <PATH> --channel <STR> [--format lines] [--callback <SCRIPT>] [--once]`.
  - Daemon: `--project <PATH>` → Runs indefinitely, processing subcommands over Unix Domain Socket.
- **Async Flow**: `#[tokio::main(flavor = "multi_thread")]` for parallelism; `select!` for muxing multiple channels.
- **Inter-Process**: Single daemon per project shares channels internally; agents invoke daemon via CLI (e.g., `hydra emit ...` proxies to daemon via Unix socket). JSON via stdin for safe data passing.
- **Security**: Env `HYDRA_KEY` for HMAC (via `hmac` crate opt-in); Project UUID adds isolation.
- **Agent Auto-Discovery** (Skills-Based, Phase 1):
  - Skills check for `.hydra/` in cwd via embedded shell commands in tool definitions
  - Skill tools source `.hydra/config.sh` to load env vars (HYDRA_UUID, HYDRA_SOCKET, HYDRA_MODE for Phase 2)
  - CLI binary proxies all commands to daemon via Unix socket path from config
  - For non-Claude agents: Simple alias shims (e.g., `alias codex-hydra='source .hydra/config.sh && codex-cli'`)
- **Mode Flow** (Phase 2):
  - Binary loads `mode` from config.toml on daemon start
  - Inject: Daemon spawns background listener, buffers urgent pulses for AgentState injection
  - Loop: Skills/wrappers call `hydra_read --since $HYDRA_TS` pre-tool, summarize in prompt
  - Hybrid: Urgent types (from config) → inject; Routine → loop read

## 6. Implementation Guidelines
- **Tech Stack**:
  - Rust 1.91.1; `edition = "2021"`.
  - Features: `default = ["tokio-sync"]`; `durable = ["sled"]`; `toon-format = ["toon"]`.
  - Async: Tokio multi-thread for subscriber tasks; no blocking calls.
  - TOON Integration: `toon = "0.1"` crate for token-efficient serialization (30-60% size reduction vs JSON).
- **Coding Standards**:
  - `cargo fmt` + `clippy`; `#[deny(unsafe_code)]`.
  - Errors: `anyhow::Result`; context with `.context("emit failed")`.
  - Tests: `#[tokio::test]` for channel flows; `cargo test -- --nocapture`.
  - Docs: `///` for pub; `cargo doc`.
- **Build/Deploy**:
  - `Cargo.toml`: `[dependencies] tokio = { ... features = ["sync"] }`; Add `toml`, `uuid`, `toon = "0.1"`.
  - Release: `cargo build --release`; cross via `cross` tool.
  - Distro: Linux/macOS only for v1. Provide:
    - Cargo: `cargo install hydra-mail`
    - Homebrew (macOS/Linux): `brew tap paddy/hydra && brew install hydra-mail`
    - Nix (flakes): `nix run .#hydra-mail` or `nix profile install .#hydra-mail`
    - GitHub Releases: tarballs with `hydra-mail` binary; install via `curl -L ... | tar -xz` then `mv hydra-mail /usr/local/bin/`
  - Project Setup: `hydra init` creates .hydra/; `hydra start` spawns daemon; CLI auto-discovers `.hydra/` in cwd.
  - MVP Scope Reminder: Use `broadcast` for fan-out and `mpsc` for point-to-point; defer `watch`/stateful channels to v1.1.

## 7. Testing and Validation
- **Unit**: `#[test]` serde + TOON encode/decode + channel send/recv (mock `tokio::time::pause`); Test config parsing/UUID scoping; Validate TOON size reduction.
- **Integration**: `#[tokio::test]` spawn CLI subprocess; assert 100% delivery in fan-out; Mock .hydra for auto-detection.
- **Perf**: Criterion: `bench_emit` for <5ms; `bench_swarm` with 10 subscribers.
- **E2E**: Mock agents: `Command::new("hydra-mail").arg("subscribe").spawn()` x10; Flood emits, verify no drops; Test daemon persistence.
- **Cross-Platform**: Actions matrix; test channel bounds under load; Verify .hydra creation on init.

## 8. Risks, Tradeoffs, and Roadmap
- **Risks**:
  - Channel overflow (bounded 1024)—Mitigate: Backpressure via send errors
  - Global state races—Use Arc<Mutex<HashMap>>
  - Daemon PID management—Handle via config
  - Skills vendor-lock to Claude—Mitigate: YAML as template for other agent MD snippets
- **Tradeoffs**:
  - In-memory ephemeral (lost on crash)—Opt-in sled for durability
  - Tokio multi-thread adds minor overhead vs. single (~200KB)
  - Single daemon per project limits to one cwd, but enables sharing
  - Hooks opt-in adds ~1-min setup—Document clearly in README to avoid friction (Phase 2)
  - Skills require one-time upload—But dramatically improve UX vs. manual CLI
- **Roadmap**:
  - **Phase 1** (v0.1.0): Skills YAML generation on init; config.sh for env; Simple emit/subscribe
  - **Phase 2** (v0.2.0): Mode support (inject/loop/hybrid); hooks opt-in; `hydra_read` tool
  - **Phase 3** (v0.3.0): SDK integration for AgentState inject; Full TOON encoding by default
  - v1.6: Sled integration for durable mpsc queues
  - v1.7: Full Unix socket proxy for daemon subcommands (no CLI overhead)
  - v2: Cross-process via Unix sockets (tokio-ipc); Windows support (named pipes)
  - Metrics: Benchmark vs. prior (expect 3x faster); crate downloads

## 9. Agent Integration and UX (Skills-Centric, Phased)

### Overview
Hydra prioritizes **Skills as the primary integration point** for AI agents, treating them as "prompt-native extensions" that teach agents how to use the mailing system. The `hydra init` command auto-generates a ready-to-upload YAML Skill (Phase 1), with optional mode layers (Phase 2) and SDK integration (Phase 3) added incrementally. Skills are **not required for operation** but dramatically improve UX—agents "just work" after a one-time upload.

### Phase 1: Skills-First MVP (Upload-Once Integration)

**Goal**: `hydra init --daemon` scaffolds everything needed for Claude to emit/subscribe via prompts.

**What Gets Generated**:
1. **`.hydra/skills/hydra-mail.yaml`** (~2KB): Tiered instructions + tools (see Section 4.1 for full structure)
   - Tier 1: "Detect .hydra, emit after actions"
   - Tier 2: `hydra_emit` and `hydra_subscribe` tools with auto-detection
   - Tier 3: Examples for common workflows

2. **`.hydra/config.sh`**: Env exports for Skills tools
   ```bash
   export HYDRA_UUID="a1b2c3d4-..."
   export HYDRA_SOCKET="/path/to/.hydra/hydra.sock"
   export HYDRA_MODE="simple"  # Phase 2: hybrid/inject/loop
   ```

**User Workflow**:
```bash
# In project root
hydra-mail init --daemon

# Output:
# ✓ Created .hydra/ with config
# ✓ Generated .hydra/skills/hydra-mail.yaml (upload to Claude)
# ✓ Generated .hydra/config.sh (auto-sourced by tools)
# ✓ Daemon started (PID: 12345)
#
# Next: Upload .hydra/skills/hydra-mail.yaml to your Claude session
```

**Agent Integration**:
- **Claude**: Upload YAML once; Skill tools handle all detection/invocation via prompts
- **Other Agents**: Simple alias shims:
  ```bash
  # .hydra/aliases.sh (generated by init)
  alias claude-hydra='source .hydra/config.sh && claude-code --skill .hydra/skills/hydra-mail.yaml'
  alias codex-hydra='source .hydra/config.sh && codex-cli'
  ```

**Why Skills-First?**:
- Prompt-driven: Agents learn usage via tiered instructions (<100 tokens trigger logic)
- Zero manual config: Tools auto-detect `.hydra` and source env vars
- Cadence-aware: Emits happen at tool-call frequency (~2-5x/session), not always-on
- Extensible: Update YAML to add modes (Phase 2) without changing binary

### Phase 2: Dual-Mode Layering (Config-Driven Adaptation)

**Goal**: Add inject/loop/hybrid modes via config; Skills adapt behavior via embedded instructions.

**Config Extension** (`config.toml`):
```toml
[integration]
mode = "hybrid"  # simple (Phase 1) | inject | loop | hybrid

[modes.hybrid]
urgent_types = ["alert", "error", "question"]  # Inject these immediately
inject_urgent = true
loop_deltas = true
```

**Skills Updates** (Tier 2):
- Add `hydra_read` tool with `--since $HYDRA_TS` for loop mode:
  ```yaml
  - name: hydra_read
    description: Read messages since last check (loop mode)
    command: |
      source .hydra/config.sh
      if [ "$HYDRA_MODE" = "loop" ] || [ "$HYDRA_MODE" = "hybrid" ]; then
        hydra-mail subscribe --project .hydra --channel "$channel" --since "${HYDRA_TS:-0}" --once
        export HYDRA_TS=$(date +%s)  # Update timestamp
      fi
  ```

- Update Tier 1 instructions:
  - "If mode=loop: Call `hydra_read` pre-tool, summarize in prompt"
  - "If mode=inject: Auto-receive urgent via background listener"
  - "If mode=hybrid: Urgent types inject, routine types read on-demand"

**Hooks (Opt-In)**:
- `config.sh` exports timestamp fn: `update_ts() { export HYDRA_TS=$(date +%s); }`
- Aliases call it: `alias codex-hydra='update_ts; source .hydra/config.sh && codex-cli --pre-tool "hydra_read"'`
- **No auto-generation of hook scripts**—keeps init clean; documented in README

**Mode UX**:
```bash
# Init with mode
hydra-mail init --daemon --mode hybrid

# Change mode later
hydra-mail modes set loop

# List available modes
hydra-mail modes list
```

### Phase 3: Advanced Layering (SDK + Full TOON)

**Goal**: Layer SDK for direct injection + full TOON encoding for max token savings.

**SDK Integration** (Opt-In Feature):
```toml
[features]
sdk = ["anthropic-sdk"]  # Enable AgentState injection
```

**Skills Update** (Hybrid/Inject Modes):
- Add SDK inject for urgent pulses:
  ```yaml
  # In hydra_listen tool (inject mode)
  command: |
    source .hydra/config.sh
    if [ "$HYDRA_MODE" = "inject" ]; then
      hydra-mail listen --project .hydra --channel team:urgent --sdk-inject
      # Binary calls anthropic_sdk::AgentState.inject(decode_toon(pulse))
    fi
  ```

**TOON Full Encoding**:
- Default: Emit/subscribe automatically encode/decode as TOON (30-60% savings)
- Binary: `src/toon.rs` handles all encoding; Skills transparent

**Roadmap Priority**: v1.1 for SDK bindings; v1.2 for full TOON by default.

### Minimal Command Surface (All Phases)
- `hydra-mail init [--daemon] [--mode <MODE>]`: Initialize project
- `hydra-mail start`: Spawn daemon (if not auto-started)
- `hydra-mail stop`: Kill daemon
- `hydra-mail status`: Show daemon status, mode, active channels
- `hydra-mail emit --project .hydra --channel <CH> --type <TYPE> --data <JSON>`: Publish
- `hydra-mail subscribe --project .hydra --channel <CH> [--once]`: Listen
- `hydra-mail modes set <MODE>` (Phase 2): Change integration mode
- `hydra-mail modes list` (Phase 2): Show available modes

### Supported OS
- Linux and macOS (Unix Domain Sockets)
- Windows deferred (Phase 2+: named pipes or TCP localhost)

### Non-Claude Agent Integration
For agents without Skill support, provide minimal shims:

```bash
# .hydra/aliases.sh (auto-generated by init)
# Source in ~/.bashrc or project shell

# For Codex CLI
alias codex-hydra='source .hydra/config.sh && codex-cli'

# For generic agents
hydra-emit() {
  if [ -d ".hydra" ]; then
    source .hydra/config.sh
    printf '%s\n' "$3" | hydra-mail emit --project .hydra --channel "$1" --type "$2" --data @-
  else
    echo "Hydra not initialized. Run: hydra-mail init --daemon" >&2
  fi
}
```

Usage: `hydra-emit repo:delta delta '{"file":"routes.py"}'`

### Why This Phased Approach?
- **Phase 1 (80% value)**: Upload YAML once, agents emit/subscribe via prompts
- **Phase 2 (Modes)**: Adds smarts (inject urgent, loop routine) without complexity
- **Phase 3 (SDK/TOON)**: Polishes for speed (<5ms inject) and token efficiency
- **Total Effort**: 3-5 days; Each phase independently useful

## Appendix: Quickstart for Engineers

### Phase 1 Implementation
1. **Setup Project**:
   ```bash
   cargo new hydra-mail --bin && cd hydra-mail
   ```

2. **Add Dependencies** (`Cargo.toml`):
   ```toml
   [dependencies]
   clap = { version = "4.5", features = ["derive"] }
   tokio = { version = "1.40", features = ["sync", "rt-multi-thread", "net", "io-util", "macros"] }
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"
   uuid = { version = "1.10", features = ["v4", "serde"] }
   toml = "0.8"
   once_cell = "1.19"
   anyhow = "1.0"
   chrono = { version = "0.4", features = ["serde"] }
   toon-format = "0.3"
   ```

3. **Implement Core**:
   - `config.rs`: Parse/generate config.toml + **Add `generate_skill_yaml()` and `generate_config_sh()`**
   - `channels.rs`: Tokio broadcast/mpsc with project_uuid scoping
   - `schema.rs`: Pulse struct with validation
   - `main.rs`: CLI with init/start/emit/subscribe commands

4. **Phase 1 Additions** (Skills Generation):
   ```rust
   // In config.rs
   impl Config {
       pub fn generate_skill_yaml(&self) -> String {
           // Template YAML with config vars (UUID, socket, topics)
           format!(r#"
   name: hydra-mail
   description: Multi-agent collaboration via pub/sub channels

   instructions: |
     Hydra Mail enables sharing state between agents. Detect .hydra in cwd.
     After actions (edits, planning), emit deltas to notify other agents.

   tools:
     - name: hydra_emit
       description: Publish a pulse to the swarm
       parameters:
         - name: channel
           type: string
         - name: type
           type: string
         - name: data
           type: string
       command: |
         if [ -d ".hydra" ]; then
           source .hydra/config.sh
           printf '%s\n' "$data" | hydra-mail emit --project .hydra --channel "$channel" --type "$type" --data @-
         else
           echo "Hydra not initialized" >&2; exit 1
         fi
   "#)
       }

       pub fn generate_config_sh(&self) -> String {
           format!(
               "export HYDRA_UUID=\"{}\"\nexport HYDRA_SOCKET=\"{}\"\nexport HYDRA_MODE=\"simple\"\n",
               self.project_uuid,
               self.socket_path.display()
           )
       }
   }
   ```

5. **Update `init` Command**:
   ```rust
   // In main.rs Commands::Init
   let config = Config::init(project_path)?;

   // Generate Skills YAML
   let skills_dir = hydra_dir.join("skills");
   fs::create_dir_all(&skills_dir)?;
   let yaml_path = skills_dir.join("hydra-mail.yaml");
   fs::write(&yaml_path, config.generate_skill_yaml())?;

   // Generate config.sh
   let sh_path = hydra_dir.join("config.sh");
   fs::write(&sh_path, config.generate_config_sh())?;

   println!("✓ Generated .hydra/skills/hydra-mail.yaml (upload to Claude)");
   println!("✓ Generated .hydra/config.sh (auto-sourced by tools)");
   ```

6. **Build and Test**:
   ```bash
   cargo build --release

   # In a test project
   cd /path/to/my-project
   ../hydra-mail/target/release/hydra-mail init --daemon
   # ✓ Created .hydra/ with config
   # ✓ Generated .hydra/skills/hydra-mail.yaml (upload to Claude)
   # ✓ Generated .hydra/config.sh
   # ✓ Daemon started (PID: 12345)
   ```

7. **Agent Integration**:
   - **Claude**: Upload `.hydra/skills/hydra-mail.yaml` to session
   - **Test Emit** (via Skill or CLI):
     ```bash
     source .hydra/config.sh
     echo '{"file":"routes.py","action":"updated"}' | \
       hydra-mail emit --project .hydra --type delta --channel repo:delta --data @-
     ```
   - **Test Subscribe**:
     ```bash
     hydra-mail subscribe --project .hydra --channel repo:delta --once
     ```

8. **Stop Daemon**:
   ```bash
   hydra-mail stop --project .hydra
   ```

### Phase 2 Additions (Modes)
- Add `src/modes.rs`: Define `InjectMode`, `LoopMode`, `HybridMode` traits
- Extend `config.rs`: Add `mode` field (default: "hybrid")
- Update Skills YAML template: Add `hydra_read` tool, mode-aware instructions
- Daemon: Load mode from config, route messages via trait dispatch

### Phase 3 Additions (SDK)
- Feature flag: `sdk = ["anthropic-sdk"]`
- `hydra_listen` tool in YAML for inject mode
- Binary: Call `anthropic_sdk::AgentState.inject()` for urgent pulses

### Testing Strategy
- **Unit**: Channel send/recv, TOON encode/decode, config parse
- **Integration**: Spawn daemon, emit from one process, subscribe from another
- **E2E**: Mock Claude session uploading Skills YAML, emitting via prompts
- **Skills**: Verify YAML generation with correct config vars
