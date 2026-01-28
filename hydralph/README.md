# Hydralph

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/0xPD33/hydra-tools)

Autonomous agent iteration loop implementing the "Ralph pattern" for PRD-driven development with fresh context each iteration.

## What is Hydralph?

Hydralph is a shell script that runs an AI agent in a loop until all tasks in a Product Requirements Document (PRD) are complete. The key insight is that **each iteration starts with fresh context** - the agent only knows what's in the files, not what happened in previous iterations.

This prevents:
- Context drift from accumulated conversation history
- Token waste from repeating the same context
- Agent confusion from stale information

**Key Features:**
- Iterative agent execution - Run agent in loop until task complete
- PRD-driven - Uses JSON PRD with user stories and acceptance criteria
- Progress tracking - Maintains progress.txt for context across iterations
- Promise tags - Detects `<promise>COMPLETE</promise>` or `<promise>BLOCKED</promise>`
- Hydra Mail integration - Emits status events with graceful degradation
- Pause/resume support - Checkpoint via `.pause` marker file
- One-story-per-iteration - Enforces focused, incremental development

## Quick Start

### Installation

Hydralph is a shell script that can be run directly or via the hydra-cli.

**Direct usage:**
```bash
# Clone the repository
git clone https://github.com/0xPD33/hydra-tools.git
cd hydra-tools/hydralph

# Make script executable
chmod +x hydralph.sh

# Ensure dependencies are installed
# Requires: jq, uuidgen, and an agent CLI (claude, etc.)
```

**Via hydra-cli:**
```bash
# Install hydra-tools
nix build .#hydra-cli

# Initialize in your project
hydra init

# This creates .hydra/ralph/ with hydralph.sh and prompt.md
```

### Usage

1. **Create your PRD** (prd.json):
```json
{
  "title": "My Project PRD",
  "userStories": [
    {
      "id": "story-1",
      "title": "Set up project structure",
      "description": "Create basic directory layout and configuration files",
      "passes": false,
      "acceptance": [
        "src/ directory exists",
        "README.md created",
        "git initialized"
      ]
    },
    {
      "id": "story-2",
      "title": "Implement core feature",
      "description": "Build the main functionality",
      "passes": false,
      "acceptance": [
        "Feature works as specified",
        "Tests pass"
      ],
      "dependsOn": ["story-1"]
    }
  ]
}
```

2. **Run the loop:**
```bash
./hydralph.sh
```

3. **Monitor progress:**
```bash
# Check current status
cat status.json

# View progress log
cat progress.txt

# Check which stories are done
jq '.userStories[] | select(.passes == true)' prd.json
```

## PRD Format

The PRD (prd.json) is a JSON file containing your project requirements:

### Structure

```json
{
  "title": "Project Title",
  "userStories": [
    {
      "id": "story-1",
      "title": "Story title",
      "description": "Detailed description of what needs to be done",
      "passes": false,
      "acceptance": [
        "Criteria 1",
        "Criteria 2"
      ],
      "dependsOn": ["story-0"]  // Optional: dependencies
    }
  ]
}
```

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | Yes | Project title |
| `userStories` | array | Yes | List of user stories |
| `id` | string | Yes | Unique story identifier |
| `title` | string | Yes | Story title |
| `description` | string | Yes | What needs to be done |
| `passes` | boolean | Yes | Set to `true` when story is complete |
| `acceptance` | array | Yes | List of acceptance criteria |
| `dependsOn` | array | No | List of story IDs that must complete first |

### Dependency Management

Stories with `dependsOn` will only be picked after all dependencies have `passes: true`:

```json
{
  "id": "story-2",
  "title": "Build API",
  "dependsOn": ["story-1"],  // Won't start until story-1 is complete
  "passes": false
}
```

## Progress Tracking

Hydralph maintains context across iterations through `progress.txt`:

### Format

```markdown
# Hydralph Progress Log
Started: 2026-01-23T16:48:00+00:00
---

## 2026-01-23T16:50:00+00:00 - story-1
- What: Set up project structure
- Files: src/main.rs, Cargo.toml, README.md
- Learnings: Needed to add libc dependency for Linux compatibility
---

## 2026-01-23T17:02:00+00:00 - story-2
- What: Implement core feature
- Files: src/lib.rs, tests/integration_test.rs
- Learnings: Used tokio::test for async tests
---
```

### Purpose

Each iteration reads `progress.txt` FIRST to learn from previous work:
- Patterns discovered
- Gotchas encountered
- Decisions made
- Files modified

This is the **only** context that persists between iterations.

## Promise Detection

Hydralph detects special "promise" tags in the agent output to control the loop:

### COMPLETE

When all stories are done, the agent outputs:
```
<promise>COMPLETE</promise>
```

Hydralph will:
- Display completion summary
- Emit `ralph:complete` event
- Exit with code 0

### BLOCKED

If the agent cannot proceed:
```
<promise>BLOCKED</promise>
Reason: Missing dependency or unclear requirements
```

Hydralph will:
- Display blocked message
- Emit `ralph:blocked` event
- Exit with code 2

### Normal Iteration

If no promise tag is output, the loop continues to the next iteration.

## Agent Workflow

Each iteration, the agent follows this workflow (defined in `prompt.md`):

1. **Read State**
   ```bash
   cat prd.json        # Find stories where passes: false
   cat progress.txt    # Learnings from previous iterations
   git log --oneline -5  # Recent commits
   ```

2. **Pick ONE Story**
   - Select highest priority story where `passes: false`
   - Only pick stories whose dependencies all have `passes: true`

3. **Implement**
   - Make small, focused changes
   - ONE story per iteration (critical!)

4. **Verify**
   ```bash
   npm run typecheck   # or equivalent
   npm test            # or equivalent
   ```
   ALL checks must pass before marking complete.

5. **Commit**
   ```bash
   git add -A
   git commit -m "feat(story-id): brief description"
   ```

6. **Update State**
   - Mark story complete in prd.json (`passes: true`)
   - Append learnings to progress.txt

7. **Check Completion**
   - If ALL stories have `passes: true`: `<promise>COMPLETE</promise>`
   - If stuck: `<promise>BLOCKED</promise>`
   - Otherwise: exit normally

## Configuration

Environment variables control hydralph behavior:

| Variable | Default | Description |
|----------|---------|-------------|
| `HYDRALPH_MAX_ITERATIONS` | 10 | Maximum iterations before stopping |
| `HYDRALPH_AGENT` | `claude` | Agent CLI to invoke |
| `HYDRALPH_FLAGS` | `--dangerously-skip-permissions` | Flags passed to agent |
| `HYDRALPH_SESSION_ID` | `<uuid>` | Unique session identifier |
| `HYDRALPH_PRD` | `./prd.json` | Path to PRD file |
| `HYDRALPH_PROGRESS` | `./progress.txt` | Path to progress log |
| `HYDRALPH_PROMPT` | `./prompt.md` | Path to agent prompt |
| `HYDRALPH_STATUS` | `./status.json` | Path to status file |

### Example

```bash
export HYDRALPH_MAX_ITERATIONS=20
export HYDRALPH_AGENT="claude"
export HYDRALPH_FLAGS=""
./hydralph.sh
```

## Hydra Mail Integration

Hydralph emits status events to Hydra Mail channels when available:

| Channel | Event | Payload |
|---------|-------|---------|
| `ralph:started` | Session started | `{session, status, iteration, max, stories}` |
| `ralph:iteration` | Iteration started | `{session, status, iteration, max, stories}` |
| `ralph:complete` | All stories passing | `{session, status, iteration, max, stories}` |
| `ralph:blocked` | Agent signaled blocked | `{session, status, iteration, max}` |
| `ralph:max-iterations` | Hit iteration limit | `{session, status, iteration, max, stories}` |

### Graceful Degradation

If `hydra-mail` is not available, events are silently ignored. The script continues normally.

### Example Payload

```json
{
  "session": "a1b2c3d4",
  "status": "running",
  "iteration": 3,
  "max": 10,
  "stories": "2/5"
}
```

## Pause/Resume

You can pause a running hydralph session:

```bash
# Create pause marker
touch .pause

# Hydralph will wait until marker is removed
# Status: "⏸️  Paused - waiting for resume..."

# Resume by removing marker
rm .pause

# Hydralph continues: "▶️  Resumed"
```

## Status File

The `status.json` file provides real-time session status:

```json
{
  "session": "a1b2c3d4",
  "status": "running",
  "iteration": 3,
  "max": 10,
  "stories": "2/5"
}
```

### Status Values

| Status | Description |
|--------|-------------|
| `started` | Session initialized |
| `running` | Iteration in progress |
| `complete` | All stories passing |
| `blocked` | Agent signaled blocked |
| `max-iterations` | Hit iteration limit |

## Via Hydra CLI

The recommended way to use hydralph is through the hydra-cli:

```bash
# Initialize in your project
hydra init

# Creates .hydra/ralph/ with:
# - prd.json (edit this with your stories)
# - progress.txt (auto-maintained)
# - hydralph.sh (the loop script)
# - prompt.md (agent instructions)
# - status.json (current status)

# Spawn a new session
hydra spawn --prd my-prd.json --max-iterations 20

# List active sessions
hydra ls

# Get session status
hydra status <session-id>

# Attach to session tmux
hydra attach <session-id>

# Pause a session
hydra pause <session-id>

# Resume a session
hydra resume <session-id>

# Kill a session
hydra kill <session-id>
```

## Directory Structure

When using hydralph directly:

```
project/
├── hydralph.sh          # The loop script
├── prompt.md            # Agent instructions
├── prd.json             # Your PRD
├── progress.txt         # Progress log (auto-generated)
├── status.json          # Current status (auto-generated)
└── .pause               # Pause marker (optional)
```

When using hydra-cli:

```
project/
├── .hydra/
│   ├── config.toml      # Hydra config
│   └── ralph/
│       ├── prd.json     # Your PRD
│       ├── progress.txt # Progress log
│       ├── hydralph.sh  # Loop script (copied)
│       ├── prompt.md    # Agent instructions (copied)
│       └── status.json  # Current status
```

## Requirements

### Required

- **jq** - JSON parsing for PRD and status files
- **Agent CLI** - `claude` or compatible agent
- **Bash** - Shell script execution

### Optional

- **hydra-mail** - For event emission (graceful degradation if missing)
- **uuidgen** - For session ID generation (fallback to timestamp)

### Installing Dependencies

```bash
# On NixOS/nix
nix develop  # Includes all dependencies

# On macOS
brew install jq

# On Debian/Ubuntu
sudo apt install jq

# Install Claude CLI
npm install -g @anthropic-ai/claude-cli
```

## Troubleshooting

### "PRD not found" Error

**Cause**: prd.json doesn't exist

**Solution**:
```bash
# Create example PRD
cat > prd.json << 'EOF'
{
  "title": "My Project",
  "userStories": [
    {
      "id": "story-1",
      "title": "First Story",
      "description": "What needs to be done",
      "passes": false,
      "acceptance": ["Criteria 1", "Criteria 2"]
    }
  ]
}
EOF
```

### "Agent not found" Error

**Cause**: Agent CLI not in PATH

**Solution**:
```bash
# Specify agent explicitly
export HYDRALPH_AGENT="/path/to/claude"
./hydralph.sh
```

### "jq required" Error

**Cause**: jq not installed

**Solution**:
```bash
# Install jq
# macOS
brew install jq

# Debian/Ubuntu
sudo apt install jq

# Nix
nix-shell -p jq
```

### Agent Completes Same Story Repeatedly

**Cause**: Agent forgot to mark `passes: true` in prd.json

**Solution**: Check that the agent is updating prd.json after each story completion.

### Loop Hits Max Iterations

**Cause**: Maximum iterations reached without completion

**Solution**:
```bash
# Check status
cat status.json

# Check progress
cat progress.txt

# Increase max iterations
export HYDRALPH_MAX_ITERATIONS=20
./hydralph.sh
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success - All stories complete |
| 1 | Max iterations reached |
| 2 | Blocked - Agent signaled it cannot proceed |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     hydralph.sh                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Configuration (env vars or defaults)                   │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Startup (check requirements, initialize files)        │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Main Loop (up to MAX_ITERATIONS)                      │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │ Check for pause marker (.pause)                 │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │ Emit status (ralph:iteration)                   │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │ Invoke agent with fresh context                 │  │  │
│  │  │ (prompt.md piped to agent CLI)                  │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │ Parse output for promise tags                   │  │  │
│  │  │ - COMPLETE → exit 0                             │  │  │
│  │  │ - BLOCKED → exit 2                              │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Key Insights

### Fresh Context Each Iteration

The "Ralph pattern" core insight: **each agent invocation starts fresh**.

Traditional agent loops accumulate conversation history, causing:
- Token waste (repeating context)
- Context drift (stale information)
- Agent confusion (contradictory instructions)

Hydralph solves this by:
1. Each iteration reads state from files (prd.json, progress.txt, git log)
2. Agent only knows what's in files, not what happened before
3. Progress tracked via commits and progress.txt, not conversation

### One Story Per Iteration

Enforcing one story per iteration ensures:
- Focused, atomic changes
- Easier debugging (git bisect friendly)
- Clearer progress tracking
- Reduced complexity per iteration

### Promise Tags

Simple, unobtrusive completion detection:
- `<promise>COMPLETE</promise>` - All stories passing
- `<promise>BLOCKED</promise>` - Cannot proceed

No complex API or protocol - just text in output.

## Platform Support

- **Linux** - Full support
- **macOS** - Full support
- **Windows** - Not supported (shell script, Unix paths)

## License

MIT - See [LICENSE](../LICENSE) for details.

## Contributing

Built by [0xPD33](https://github.com/0xPD33)

Issues and PRs welcome at [https://github.com/0xPD33/hydra-tools](https://github.com/0xPD33/hydra-tools)

## Related Projects

- **hydra-cli** - Unified CLI for Hydra orchestrator
- **hydra-orchestrator** - Session management library
- **hydra-mail** - Pub/sub messaging for coordination
