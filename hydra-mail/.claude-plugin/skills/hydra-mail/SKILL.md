---
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
| Fixed auth bug | repo:delta | delta | `{"action":"fixed","target":"auth.py","impact":"login validates tokens"}` |
| Refactored DB | repo:delta | delta | `{"action":"refactored","target":"db/","reason":"performance","impact":"query API changed"}` |
| Tests failing | team:alert | alert | `{"action":"test_failure","target":"integration","count":3}` |
| Need input | team:question | question | `{"action":"question","topic":"API design","details":"REST or GraphQL?"}` |
| Task done | team:status | status | `{"action":"completed","task":"user auth","duration":"2h"}` |

## Common Mistakes

❌ **Emitting before action**: `"planning to update routes.py"`
✅ **Emit after**: `"updated routes.py with new auth flow"`

❌ **Vague messages**: `{"file":"routes.py"}`
✅ **Include context**: `{"action":"updated","target":"routes.py","reason":"fix CVE","impact":"login flow changed"}`

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
