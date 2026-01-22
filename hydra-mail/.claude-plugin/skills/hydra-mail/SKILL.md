---
name: hydra-mail
description: >
  Multi-agent pub/sub coordination for AI agents working in parallel. Use when:
  (1) Project has .hydra/ directory initialized,
  (2) Working with multiple agents or parallel tasks,
  (3) Need to coordinate, communicate, or share state with other agents,
  (4) Starting a session and should check what other agents did,
  (5) Finishing work and should notify other agents.
  Triggers: multi-agent, coordinate, other agents, parallel agents, emit, broadcast, hydra-mail, check messages, agent coordination.
hooks:
  SessionStart:
    - hooks:
        - type: command
          command: "hydra-mail hook session-start --project ."
  Stop:
    - hooks:
        - type: command
          command: "hydra-mail hook stop --project ."
---

# Hydra Mail

Multi-agent pub/sub for coordinating parallel AI agents.

## Automatic Behavior

- **On session start**: Checks for recent messages from other agents
- **On stop**: Reminds you to emit a summary of your work

## Commands

Check for messages:
```bash
hydra-mail subscribe --channel repo:delta --once
```

Emit your work:
```bash
hydra-mail emit --channel repo:delta --type delta \
  --data '{"action":"<verb>","target":"<file>","summary":"<what changed>"}'
```

## Channels

| Channel | Use For |
|---------|---------|
| `repo:delta` | Code changes, fixes, refactoring |
| `team:status` | Task completion, progress |
| `team:alert` | Errors, blockers |
| `team:question` | Questions needing input |

## Message Format

```json
{"action":"<verb>","target":"<what>","summary":"<impact>"}
```

**Actions**: `fixed`, `added`, `updated`, `refactored`, `completed`, `investigating`, `blocked`

## Examples

After fixing a bug:
```bash
hydra-mail emit --channel repo:delta --type delta \
  --data '{"action":"fixed","target":"auth.py","summary":"token validation works"}'
```

After completing work:
```bash
hydra-mail emit --channel repo:delta --type status \
  --data '{"action":"completed","summary":"implemented user auth feature"}'
```

## Setup

If not initialized:
```bash
hydra-mail init --daemon
```
