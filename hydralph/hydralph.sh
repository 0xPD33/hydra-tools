#!/usr/bin/env bash
# hydralph - Enhanced Ralph loop for Hydra Tools
# Close to OG, but with structured output and hydra-mail integration

set -e

# ═══════════════════════════════════════════════════════════════════════════
# Configuration (from env or defaults)
# ═══════════════════════════════════════════════════════════════════════════

MAX_ITERATIONS=${HYDRALPH_MAX_ITERATIONS:-10}
AGENT_CLI=${HYDRALPH_AGENT:-"claude"}
AGENT_FLAGS=${HYDRALPH_FLAGS:-"--dangerously-skip-permissions"}
SESSION_ID=${HYDRALPH_SESSION_ID:-$(uuidgen 2>/dev/null | tr '[:upper:]' '[:lower:]' | cut -c1-8 || echo "manual-$(date +%s)")}
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRD_FILE="${HYDRALPH_PRD:-$SCRIPT_DIR/prd.json}"
PROGRESS_FILE="${HYDRALPH_PROGRESS:-$SCRIPT_DIR/progress.txt}"
PROMPT_FILE="${HYDRALPH_PROMPT:-$SCRIPT_DIR/prompt.md}"
STATUS_FILE="${HYDRALPH_STATUS:-$SCRIPT_DIR/status.json}"

# ═══════════════════════════════════════════════════════════════════════════
# Hydra-mail integration (optional, silent fail if unavailable)
# ═══════════════════════════════════════════════════════════════════════════

emit() {
    local channel="$1"
    local payload="$2"
    if command -v hydra-mail &>/dev/null; then
        hydra-mail emit --channel "$channel" --type status --data "$payload" 2>/dev/null || true
    fi
}

# ═══════════════════════════════════════════════════════════════════════════
# Status helpers
# ═══════════════════════════════════════════════════════════════════════════

status_json() {
    local status="$1"
    local iteration="$2"
    local extra="$3"
    echo "{\"session\":\"$SESSION_ID\",\"status\":\"$status\",\"iteration\":$iteration,\"max\":$MAX_ITERATIONS${extra:+,$extra}}"
}

write_status() {
    local payload="$1"
    printf "%s\n" "$payload" > "$STATUS_FILE"
}

count_stories() {
    if [[ -f "$PRD_FILE" ]]; then
        local total=$(jq '.userStories | length' "$PRD_FILE" 2>/dev/null || echo 0)
        local done=$(jq '[.userStories[] | select(.passes == true)] | length' "$PRD_FILE" 2>/dev/null || echo 0)
        echo "$done/$total"
    else
        echo "0/0"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════
# Startup
# ═══════════════════════════════════════════════════════════════════════════

echo "╔═══════════════════════════════════════════════════════════════════════╗"
echo "║  HYDRALPH - Session: $SESSION_ID"
echo "║  Agent: $AGENT_CLI | Max iterations: $MAX_ITERATIONS"
echo "║  PRD: $PRD_FILE"
echo "╚═══════════════════════════════════════════════════════════════════════╝"

# Check requirements
[[ -f "$PRD_FILE" ]] || { echo "❌ PRD not found: $PRD_FILE"; exit 1; }
[[ -f "$PROMPT_FILE" ]] || { echo "❌ Prompt not found: $PROMPT_FILE"; exit 1; }
command -v "$AGENT_CLI" &>/dev/null || { echo "❌ Agent not found: $AGENT_CLI"; exit 1; }
command -v jq &>/dev/null || { echo "❌ jq required"; exit 1; }

# Initialize progress file if missing
[[ -f "$PROGRESS_FILE" ]] || echo "# Hydralph Progress Log\nStarted: $(date -Iseconds)\n---" > "$PROGRESS_FILE"

status_payload=$(status_json "started" 0 "\"stories\":\"$(count_stories)\"")
write_status "$status_payload"
emit "ralph:started" "$status_payload"

# ═══════════════════════════════════════════════════════════════════════════
# Main loop
# ═══════════════════════════════════════════════════════════════════════════

for i in $(seq 1 $MAX_ITERATIONS); do
    # Check for pause marker (from hydra pause)
    PAUSE_MARKER="$SCRIPT_DIR/.pause"
    if [[ -f "$PAUSE_MARKER" ]]; then
        echo ""
        echo "⏸️  Paused - waiting for resume..."
        while [[ -f "$PAUSE_MARKER" ]]; do
            sleep 1
        done
        echo "▶️  Resumed"
    fi

    stories=$(count_stories)

    echo ""
    echo "═══════════════════════════════════════════════════════════════════════"
    echo "  Iteration $i/$MAX_ITERATIONS | Stories: $stories"
    echo "═══════════════════════════════════════════════════════════════════════"

    status_payload=$(status_json "running" $i "\"stories\":\"$stories\"")
    write_status "$status_payload"
    emit "ralph:iteration" "$status_payload"

    # ─────────────────────────────────────────────────────────────────────────
    # Fresh agent invocation (the core Ralph insight)
    # ─────────────────────────────────────────────────────────────────────────

    ITER_START=$(date +%s)

    OUTPUT=$(cat "$PROMPT_FILE" | $AGENT_CLI $AGENT_FLAGS 2>&1 | tee /dev/stderr) || true

    ITER_DURATION=$(($(date +%s) - ITER_START))

    # ─────────────────────────────────────────────────────────────────────────
    # Completion detection
    # ─────────────────────────────────────────────────────────────────────────

    if echo "$OUTPUT" | grep -q "<promise>COMPLETE</promise>"; then
        echo ""
        echo "╔═══════════════════════════════════════════════════════════════════════╗"
        echo "║  ✅ COMPLETE - All stories passing!"
        echo "║  Iterations: $i | Duration: ${ITER_DURATION}s last iter"
        echo "╚═══════════════════════════════════════════════════════════════════════╝"

        status_payload=$(status_json "complete" $i "\"stories\":\"$(count_stories)\"")
        write_status "$status_payload"
        emit "ralph:complete" "$status_payload"
        exit 0
    fi

    # ─────────────────────────────────────────────────────────────────────────
    # Check for blocked/stuck signals from agent
    # ─────────────────────────────────────────────────────────────────────────

    if echo "$OUTPUT" | grep -q "<promise>BLOCKED</promise>"; then
        echo ""
        echo "╔═══════════════════════════════════════════════════════════════════════╗"
        echo "║  ⚠️  BLOCKED - Agent signaled it cannot proceed"
        echo "╚═══════════════════════════════════════════════════════════════════════╝"

        status_payload=$(status_json "blocked" $i)
        write_status "$status_payload"
        emit "ralph:blocked" "$status_payload"
        exit 2
    fi

    # Brief pause between iterations
    sleep 2
done

# ═══════════════════════════════════════════════════════════════════════════
# Max iterations reached
# ═══════════════════════════════════════════════════════════════════════════

echo ""
echo "╔═══════════════════════════════════════════════════════════════════════╗"
echo "║  ⚠️  MAX ITERATIONS ($MAX_ITERATIONS) - Stopping"
echo "║  Stories: $(count_stories)"
echo "╚═══════════════════════════════════════════════════════════════════════╝"

status_payload=$(status_json "max-iterations" $MAX_ITERATIONS "\"stories\":\"$(count_stories)\"")
write_status "$status_payload"
emit "ralph:max-iterations" "$status_payload"
exit 1
