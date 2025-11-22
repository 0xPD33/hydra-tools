use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub project_uuid: Uuid,
    pub socket_path: PathBuf,
    pub default_topics: Vec<String>,
}

impl Config {
    pub fn init(project_root: &Path) -> Result<Self> {
        let hydra_dir = project_root.join(".hydra");
        fs::create_dir_all(&hydra_dir).context("Failed to create .hydra directory")?;
        fs::set_permissions(&hydra_dir, fs::Permissions::from_mode(0o700))
            .context("Failed to set .hydra permissions")?;

        let project_uuid = Uuid::new_v4();
        // Use absolute path for socket to avoid issues with daemon cwd
        let socket_path = hydra_dir.canonicalize()
            .unwrap_or_else(|_| hydra_dir.to_path_buf())
            .join("hydra.sock");

        let config = Config {
            project_uuid,
            socket_path,
            default_topics: vec![
                "repo:delta".to_string(),
                "agent:presence".to_string(),
            ],
        };

        let config_path = hydra_dir.join("config.toml");
        let toml_str = toml::to_string(&config).context("Failed to serialize config to TOML")?;
        let mut file = File::create(&config_path).context("Failed to create config.toml")?;
        file.write_all(toml_str.as_bytes()).context("Failed to write config.toml")?;

        Ok(config)
    }

    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".hydra").join("config.toml");
        let config_str = fs::read_to_string(&config_path).context("Failed to read config.toml")?;
        toml::from_str(&config_str).context("Failed to parse config.toml")
    }

    /// Generate Skills YAML for Claude Code integration
    pub fn generate_skill_yaml(&self) -> String {
        format!(r#"---
name: hydra-mail
description: Use when working on projects with multiple AI agents that need to coordinate and share state changes - provides lightweight pub/sub messaging with 30-60% token savings via TOON encoding. Emit completed actions to channels (repo:delta for code changes, team:alert for errors, team:question for coordination needs).
---

# Hydra Mail - Multi-Agent Pub/Sub

## Core Principle
**Emit state deltas after completing actions.** Messages use TOON (Token-Oriented Object Notation) for automatic token efficiency.

## When to Emit

**After these actions** (not during):
- File edits, refactoring, architecture changes → `repo:delta`
- Test results, build status → `team:status`
- Errors, warnings, blockers → `team:alert`
- Questions needing input → `team:question`

**Never emit:**
- Before changes (no speculation)
- During partial work (wait until complete)
- Every keystroke (batch related changes)

## Tools

### hydra_emit
Broadcast a state change to other agents (auto-encodes to TOON)

**Parameters:**
- `channel` (required): Namespace:topic format - `repo:delta`, `team:alert`, `team:status`, `team:question`
- `type` (required): Action type - `delta`, `status`, `alert`, `question`, `ack`
- `data` (required): JSON with `action` (what), `target` (where), `reason` (why), `impact` (effects)

**Command:**
```bash
if [ -d ".hydra" ]; then
  source .hydra/config.sh
  printf '%s\n' "$data" | hydra-mail emit --project . --channel "$channel" --type "$type" --data @-
else
  echo "Hydra not initialized. Run: hydra-mail init --daemon" >&2
  exit 1
fi
```

### hydra_subscribe
Listen for messages from other agents (auto-decodes TOON)

**Parameters:**
- `channel` (required): Channel to subscribe to
- `once` (boolean, default true): Get one message and exit (true) or stream continuously (false)

**Command:**
```bash
if [ -d ".hydra" ]; then
  source .hydra/config.sh
  if [ "$once" = "true" ]; then
    hydra-mail subscribe --project . --channel "$channel" --once
  else
    hydra-mail subscribe --project . --channel "$channel"
  fi
else
  echo "Hydra not initialized" >&2
  exit 1
fi
```

## Quick Reference

| Scenario | Channel | Type | Data Example |
|----------|---------|------|--------------|
| Fixed auth bug | repo:delta | delta | `{{"action":"fixed","target":"auth.py","impact":"login validates tokens"}}` |
| Refactored DB | repo:delta | delta | `{{"action":"refactored","target":"db/","reason":"performance","impact":"query API changed"}}` |
| Tests failing | team:alert | alert | `{{"action":"test_failure","target":"integration","count":3}}` |
| Need input | team:question | question | `{{"action":"question","topic":"API design","details":"REST or GraphQL?"}}` |
| Task done | team:status | status | `{{"action":"completed","task":"user auth","duration":"2h"}}` |

## Common Mistakes

❌ **Emitting before action**: `"planning to update routes.py"`
✅ **Emit after**: `"updated routes.py with new auth flow"`

❌ **Vague messages**: `{{"file":"routes.py"}}`
✅ **Include context**: `{{"action":"updated","target":"routes.py","reason":"fix CVE","impact":"login flow changed"}}`

❌ **Wrong channel**: Using repo:delta for questions
✅ **Match intent**: team:question for questions, repo:delta for code changes

❌ **Every line change**
✅ **Batch related changes**, emit when logical unit complete

## Technical Notes

- **Replay buffer**: Last 100 messages per channel
- **Late subscribers**: Receive full history automatically
- **Latency**: <5ms message delivery
- **Isolation**: Project-scoped by UUID
- **TOON savings**: 30-60% smaller than JSON
- **Persistence**: In-memory only (ephemeral)

## Project Config
- UUID: {}
- Socket: {}
- Default channels: repo:delta, agent:presence
"#, self.project_uuid, self.socket_path.display())
    }

    /// Generate config.sh for shell integration
    pub fn generate_config_sh(&self) -> String {
        format!(r#"#!/bin/bash
# Hydra Mail configuration - auto-generated by hydra-mail init
# Source this file to set environment variables for Hydra tools

export HYDRA_UUID="{}"
export HYDRA_SOCKET="{}"
export HYDRA_FORMAT="toon"
"#, self.project_uuid, self.socket_path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_serialize_deserialize() {
        let config = Config {
            project_uuid: Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap(),
            socket_path: PathBuf::from(".hydra/hydra.sock"),
            default_topics: vec!["repo:delta".to_string(), "agent:presence".to_string()],
        };

        let toml_str = toml::to_string(&config).unwrap();
        let loaded: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.project_uuid, loaded.project_uuid);
        assert_eq!(config.socket_path, loaded.socket_path);
        assert_eq!(config.default_topics, loaded.default_topics);
    }

    #[test]
    fn test_init_load() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();

        let config = Config::init(project_root).unwrap();
        assert!(project_root.join(".hydra").exists());

        let loaded = Config::load(project_root).unwrap();
        assert_eq!(config.project_uuid, loaded.project_uuid);
        assert!(config.socket_path.starts_with(project_root.join(".hydra")));
    }
}
