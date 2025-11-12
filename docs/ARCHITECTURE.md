# Hydra Mail: Tokio Channels-Based Architecture Design Document

## Document Metadata
- **Title**: Hydra Mail - Lightweight In-Memory Pub/Sub Protocol for Local Agent State Broadcasting (Tokio Channels Implementation)
- **Version**: 1.2.0 (Project Integration Update)
- **Date**: November 12, 2025
- **Author**: Grok (xAI Assistant) - Adapted from prior Rust spec, incorporating Tokio channels and project-aware init
- **Status**: Ready for Engineering Handover
- **Audience**: Lead Engineer / Rust Development Team
- **Dependencies**: Rust 1.91.1 (stable), Cargo, Tokio 1.40.0+ (minimal features)
- **Estimated Effort**: 3-4 engineer-days for MVP (CLI binary with Tokio integration and project init); 5-6 days for tests + benchmarks
- **Rationale for Tokio Channels**: Based on 2025 ecosystem research (Rust 1.91.1 stable; Tokio 1.40 updates for async ergonomics), Tokio's built-in `broadcast` and `mpsc` channels provide zero-dependency, in-memory pub/sub ideal for local (same-machine) agent communication. Sub-μs latency, thread-safe, and scales to 10-20 subscribers without external brokers like Redis—aligning with local-only needs (no network/distributed).

## 1. Executive Summary
This iteration refines Hydra Mail to a pure in-memory, Tokio channels-driven architecture, eliminating external deps (e.g., Redis) for ultimate lightness (~1MB binary). Agents (e.g., Claude Code via Skill, Codex CLI wrappers) broadcast "pulses" (TOON-encoded state deltas) via `tokio::sync::broadcast` for fan-out, with `mpsc` for point-to-point handoffs (e.g., polls/acks). To enable seamless multi-agent collaboration, Hydra now includes project-specific initialization: `hydra init` creates a `.hydra` directory with configuration (e.g., project ID, default topics), spawning an optional persistent daemon for shared channels. Agents automatically detect the `.hydra` setup in the project root and register (subscribe/emit) via the CLI binary, making any project "collaboration-ready" without manual config. This supports collaborative coding swarms on shared codebases, reducing coordination latency to <5ms while keeping token use <200/session via unchanged Skill tiering.

**Key Benefits**:
- **Local-Optimized**: In-memory only—no files/pipes/IO for core pub/sub; optional sled for persistence.
- **Project-Aware**: `init` scaffolds per-project state; auto-detection for agents.
- **Performant**: Tokio's zero-copy channels enable 1M+ events/sec in benchmarks; async non-blocking.
- **Lightweight**: Single binary; Tokio features=["sync"] (~300KB add); no runtime overhead.
- **Token-Efficient**: TOON format reduces message payload by 30-60% vs JSON, cutting LLM context usage.
- **Simple**: CLI commands spawn channels; Skill tools exec binary for emits; daemon for persistence.

MVP Scope: Tokio broadcast as default broker; Claude Skill updated for auto-detection. Future: Hybrid with sled for durable queues.

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
│   ├── config.toml  # project_uuid, default_topics, socket = /run/user/1000/hydra/<uuid>.sock (fallback: /tmp/hydra-<uuid>.sock)
│   └── daemon.pid   # If daemon mode
├── hydra-mail binary (in PATH or local)
└── Agents (Claude/Codex)

[Claude Code Session (Publisher)]
  ├── Skill: Detect .hydra → Read config → Craft TOON-encoded delta
  └── Tool: shell("hydra-mail emit --project .hydra --type delta --data '{...}' --channel 'repo:stock' --format toon")
          ↓ (Exec Binary, Scoped to Project)
[Rust CLI Binary (In-Memory Broker, Project-Scoped)]
  ├── src/main.rs: clap args → tokio::spawn(broker_task); Load .hydra/config
  ├── src/channels.rs: Static HashMap<String, (Sender, Receiver)> keyed by project_uuid + topic
  │   ├── Broadcast: tokio::sync::broadcast::channel::<String>(1024)  // Fan-out pulses
  │   └── MPSC: tokio::sync::mpsc::channel::<String>(1024)  // Point-to-point acks
  ├── src/schema.rs: serde for Pulse validation
  └── src/toon.rs: TOON encoding/decoding for token-efficient payloads
          ↓ (Async Non-Blocking, Daemon if --daemon)
[Subscriber Agents (e.g., Codex Wrapper)]
  ├── Auto-Detect: If .hydra exists, shell("hydra-mail subscribe --project .hydra --channel 'repo:stock'")
  └── Tokio Loop: select! { msg = rx.recv() => handle(msg) }
[Optional: Sled Queue] → Feature: Append for durability (v1.1)
```

- **Data Flow**:
  1. **Init**: `hydra init` in project root → Create .hydra/dir, config.toml (UUID, defaults), optional --daemon to spawn persistent broker.
  2. **Emit**: Agent detects .hydra → Parse args + config → CLI connects to daemon via Unix socket and proxies `emit`; daemon TOON-encodes payload and executes `tx.send(toon_data).await?;` (broadcast for pulses, mpsc for targeted; scoped to project_uuid).
  3. **Subscribe/Auto-Register**: Agent checks for .hydra → CLI connects to daemon via Unix socket and requests a subscription; Return receiver stream; async loop prints/callbacks (e.g., spawn reactor task).
  4. **Cleanup**: Drop on binary exit; daemon persists until killed (e.g., `hydra stop`); optional sled flush.
  5. **Resilience**: Channel bounds (1024) prevent OOM; `Result` for send errors.

### Key Assumptions
- Agents spawn via CLI wrappers or Skills that check for .hydra in cwd and invoke with --project flag.
- Tokio runtime per binary instance; multi-instance via OS processes, but shared via project daemon (single process per project).
- Pulses as TOON-encoded strings (30-60% smaller than JSON); serde for parse, TOON for token-efficient transport.
- Project scoping via UUID in config.toml to isolate channels (e.g., key = format!("{project_uuid}:{topic}")).
- v1 targets Linux and macOS using Unix Domain Sockets; Windows support deferred.

## 4. Core Components
### 4.1 Claude Skill (YAML: Updated for Auto-Detection, ~2.5KB)
- As prior spec: Tiered instructions/tools invoke Rust binary (e.g., `./hydra-mail emit ...`).
- New: Pre-emit/subscribe step: Check if .hydra exists in cwd; if yes, add `--project .hydra` to commands and read config for default channels.
- Data Input: Prefer stdin for JSON payloads to avoid shell escaping: `printf '%s\n' '{"key":"value"}' | hydra-mail emit --project .hydra --channel repo:delta --type delta --data @-`.
- Efficiency: Channels offload all I/O—Skill focuses on trigger logic (<100 tokens emit decision); CLI provides clear exit codes (0=success, 1=error) with actionable stderr (e.g., "Run: hydra init --daemon").

### 4.2 Rust CLI Binary (Core: ~300 LoC)
- **Role**: Channel manager—async tasks for pub/sub, with clap for commands; project-aware via .hydra.
- **Crate Structure**:
  ```
  src/
  ├── main.rs          # #[tokio::main] entry; clap::Parser; Load .hydra/config
  ├── schema.rs        # #[derive(serde::Deserialize)] pub struct Pulse { ... }
  ├── channels.rs      // Core: once_cell::sync::Lazy<HashMap<String, BroadcastChannel>>; Scoped by project
  ├── broadcast.rs     // Impl Broker for tokio::sync::broadcast
  ├── mpsc.rs          // Impl for tokio::sync::mpsc (targeted)
  ├── config.rs        // Parse .hydra/config.toml; Generate UUID
  └── lib.rs           // Exports + tests
  Cargo.toml           // Tokio + minimal deps + toml crate
  ```
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
  - Init: `[--daemon]` to spawn persistent broker.
  - Emit: `--project <PATH> --type <PulseType> --data <JSON or @-> --channel <STR> [--target <AGENT_ID> for mpsc]`.
  - Subscribe: `--project <PATH> --channel <STR> [--format lines] [--callback <SCRIPT>] [--once]`.
  - Daemon: `--project <PATH>` → Runs indefinitely, processing subcommands over Unix Domain Socket.
- **Async Flow**: `#[tokio::main(flavor = "multi_thread")]` for parallelism; `select!` for muxing multiple channels.
- **Inter-Process**: Single daemon per project shares channels internally; agents invoke daemon via CLI (e.g., `hydra emit ...` proxies to daemon via Unix socket). JSON via stdin for safe data passing.
- **Security**: Env `HYDRA_KEY` for HMAC (via `hmac` crate opt-in); Project UUID adds isolation.
- **Agent Auto-Discovery**: CLI and agent wrappers first look for `.hydra/` in cwd (or parent dirs), read `config.toml` to discover `project_uuid` and `socket` path, and proxy commands to the daemon transparently.

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
- **Risks**: Channel overflow (bounded 1024)—Mitigate: Backpressure via send errors. Global state races—Use Arc<Mutex<HashMap>>. Daemon PID management—Handle via config.
- **Tradeoffs**: In-memory ephemeral (lost on crash)—Opt-in sled. Tokio multi-thread adds minor overhead vs. single (~200KB). Single daemon per project limits to one cwd, but enables sharing.
- **Roadmap**:
  - v1.1: Sled integration for durable mpsc queues.
  - v1.2: Full Unix socket proxy for daemon subcommands (no CLI overhead).
  - v2: Cross-process via Unix sockets (tokio-ipc).
  - Metrics: Benchmark vs. prior (expect 3x faster); crate downloads.

## 9. Agent Integration and UX (MVP)
- **Auto-Discovery (Simple)**:
  - Agents/CLI check for `.hydra/` in the current directory (and parents).
  - If found, read `.hydra/config.toml` for `project_uuid` and `socket` and proxy all commands to the daemon.
  - If missing, show a helpful hint: `Run 'hydra init --daemon' in your project root.`
- **Supported OS**: Linux and macOS using Unix Domain Sockets. Windows support is deferred.
- **Minimal Command Surface**:
  - `hydra init [--daemon]`, `hydra start`, `hydra stop`, `hydra status`
  - `hydra emit --project .hydra --channel <CH> --type <TYPE> --data <JSON>`
  - `hydra subscribe --project .hydra --channel <CH>`
  - Optional: `hydra tap` to view live traffic for debugging.
- **MVP Channel Semantics**:
  - Use `broadcast` for fan-out events; use `mpsc` for targeted acks/RPC.
  - Stateful channels (`watch`) deferred to v1.1 to keep MVP lean.

### 9.1 Claude Skill (YAML) – Minimal Integration
Agents can adopt Hydra with a shell-first approach. Example Skill definition invokes the local CLI if `.hydra/` is present:

```yaml
name: hydra_emit
description: Publish a Hydra pulse to the project bus (if .hydra exists)
parameters:
  - name: channel
    type: string
  - name: type
    type: string
  - name: data
    type: string
command: |
  if [ -d ".hydra" ]; then
    hydra-mail emit --project .hydra --channel "$channel" --type "$type" --data "$data"
  else
    echo "Hydra: .hydra not found in this directory; run 'hydra init --daemon' first." >&2
  fi
```

### 9.2 Generic Agent Wrapper (POSIX Shell)
For non-Claude agents (e.g., Codex CLI wrappers), a tiny wrapper adds Hydra awareness without code changes:

```bash
# hydra-emit.sh
set -euo pipefail
if [ -d ".hydra" ]; then
  hydra-mail emit --project .hydra --channel "${1:?channel}" --type "${2:?type}" --data "${3:?json}"
else
  echo "Hydra not initialized here. Run: hydra init --daemon" >&2
fi
```

Include this script in PATH and call `hydra-emit.sh repo:delta delta '{"file":"..."}'`.

## Appendix: Quickstart for Engineers
1. `cargo new hydra-mail --bin && cd hydra-mail`.
2. `Cargo.toml`: Add tokio/serde/clap/once_cell/uuid/toml.
3. Impl `config.rs` for .hydra setup; Update channels.rs with project_uuid key.
4. Build: `cargo build --release`.
5. In a project dir (e.g., /path/to/my-project): `./target/release/hydra-mail init --daemon` → Creates .hydra/ and spawns daemon.
6. Launch Agent (e.g., Claude): Skill auto-detects .hydra, emits: `./target/release/hydra-mail emit --project .hydra --type delta --data '{"file":"routes.py"}' --channel repo:stock`.
7. Launch Subscriber (e.g., Codex wrapper): Auto-invoke `./target/release/hydra-mail subscribe --project .hydra --channel repo:stock` → See pulses in loop.
8. Stop: `./target/release/hydra-mail stop --project .hydra`.
9. For non-daemon: Direct invokes work but channels not shared across processes—use daemon for multi-agent.
