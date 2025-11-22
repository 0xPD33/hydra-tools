# Hydra Mail – Current Architecture (v0.1.0)

This document describes what is implemented **today**. The former architecture proposal now lives in `docs/SPEC.md`.

## Scope
- Local-only, single-host pub/sub for agents.
- Tokio broadcast channels with in-memory replay (no durability).
- Unix Domain Socket (UDS) daemon per project; no network transport.
- TOON-only encoding; no JSON fallback.

## Components
- **CLI (src/main.rs)**: Commands `init`, `start`, `emit`, `subscribe`, `status`, `stop`.
  - `init`: creates `.hydra/`, writes `config.toml`, generates `config.sh` and a Skill YAML, optionally spawns daemon by copying the binary to `.hydra/hydra-daemon` and starting it.
  - `start`: runs the daemon in the foreground (the `--daemon` flag is currently ignored here).
  - `emit`: encodes a pulse to TOON, base64s it, and sends it over the UDS to the daemon.
  - `subscribe`: opens a UDS connection and streams TOON strings from the daemon.
  - `status`/`stop`: file-based checks and PID kill; no RPC to the daemon.
- **Daemon (src/main.rs::handle_conn)**: Listens on the UDS, accepts `emit` and `subscribe` JSON commands.
- **Channels (src/channels.rs)**: Per-project `broadcast` channels keyed by `(project_uuid, topic)` with a 100-message replay buffer; no mpsc/targeted queues.
- **Config (src/config.rs)**: `config.toml` stores `project_uuid`, `socket_path`, and `default_topics`. `config.sh` exports `HYDRA_UUID`, `HYDRA_SOCKET`, `HYDRA_FORMAT=toon`. Skill YAML is templated here.
- **Schema/Format**:
  - CLI constructs a JSON pulse with `id`, `timestamp`, `type`, `channel`, `data`, optional `metadata`.
  - Pulse is TOON-encoded with safe key folding, limited to 1KB, then base64-wrapped for the daemon command.
  - Daemon stores and replays the TOON string; subscribers receive TOON text (no decode/pretty-print).

## Message Flow
1) **emit**: CLI → build JSON pulse → TOON encode → base64 → UDS `{"cmd":"emit","channel":..., "data":...}` → daemon decodes base64 → stores TOON in replay buffer → broadcasts to Tokio channel.
2) **subscribe**: CLI → UDS `{"cmd":"subscribe","channel":...}` → daemon sends replay buffer (TOON strings) then live broadcasts as TOON strings; client prints lines.

## Limits and Defaults
- Replay buffer: 100 messages per channel.
- Channel capacity: Tokio broadcast bound 1024.
- Message size: 1KB (TOON string length) enforced client-side.
- Default topics: `repo:delta`, `agent:presence`.
- Supported format: `toon` only.
- OS: Unix-like (UDS); Windows not supported.

## Known Gaps vs. Spec
- No mpsc/targeted acks; only broadcast channels exist.
- No mode system (inject/loop/hybrid), no SDK injection, no sled/durability.
- Subscribers receive TOON strings; there is no JSON/TOON decoding on the client path.
- `start --daemon` flag is ignored; backgrounding only happens via `init --daemon`.
- `status` does not query the daemon; it inspects files and `ps`.
