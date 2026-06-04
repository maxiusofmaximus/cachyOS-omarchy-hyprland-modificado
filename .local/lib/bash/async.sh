#!/usr/bin/env bash
# async.sh — TypeScript-inspired async primitives for bash.
# Source this file: source ~/.local/lib/bash/async.sh
# Requires: bash 4.3+

# --- Internal ---

_async_dir() { echo "/tmp/bash_async_${$}_${1}"; }

_async_cleanup() { rm -rf "$(_async_dir "$1")" 2>/dev/null; }

# --- API ---

# async <name> <cmd...>
# Runs cmd in background. Result stored under /tmp/bash_async_PID_name/.
async() {
    local name="$1"; shift
    local dir; dir="$(_async_dir "$name")"
    mkdir -p "$dir"
    ( "$@" > "$dir/stdout" 2> "$dir/stderr"; echo $? > "$dir/exit" ) &
    echo $! > "$dir/pid"
}

# await <name>
# Blocks until <name> finishes. Prints stdout, returns its exit code.
await() {
    local name="$1"
    local dir; dir="$(_async_dir "$name")"
    [ -f "$dir/pid" ] || { echo "await: unknown promise '$name'" >&2; return 1; }

    wait "$(cat "$dir/pid")" 2>/dev/null
    local code; code=$(cat "$dir/exit" 2>/dev/null || echo 1)
    cat "$dir/stdout" 2>/dev/null
    _async_cleanup "$name"
    return "$code"
}

# await_all <name...>
# Waits for all promises. Returns 0 only if every one succeeded.
await_all() {
    local failed=0
    for name in "$@"; do await "$name" || failed=$?; done
    return $failed
}

# await_allSettled <name...>
# Waits for all regardless of exit codes. Never fails.
await_allSettled() {
    for name in "$@"; do await "$name" 2>/dev/null || true; done
}

# await_race <name...>
# Returns when the FIRST promise resolves. Cancels the rest.
await_race() {
    while true; do
        for name in "$@"; do
            local dir; dir="$(_async_dir "$name")"
            if [ -f "$dir/exit" ]; then
                local code; code=$(cat "$dir/exit")
                cat "$dir/stdout" 2>/dev/null
                _async_cleanup "$name"
                for other in "$@"; do
                    [ "$other" = "$name" ] && continue
                    local opid; opid=$(cat "$(_async_dir "$other")/pid" 2>/dev/null)
                    [ -n "$opid" ] && kill "$opid" 2>/dev/null
                    _async_cleanup "$other"
                done
                return "$code"
            fi
        done
        sleep 0.02
    done
}

# with_timeout <ms> <cmd...>
# Runs cmd; kills it and returns 124 if it exceeds <ms> milliseconds.
with_timeout() {
    local ms="$1"; shift
    local deadline=$(( $(date +%s%3N) + ms ))
    "$@" &
    local pid=$!
    while kill -0 "$pid" 2>/dev/null; do
        [ "$(date +%s%3N)" -ge "$deadline" ] && {
            kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null; return 124
        }
        sleep 0.02
    done
    wait "$pid"
}

# poll_until <ms_timeout> <condition_cmd...>
# Calls condition every 20ms until it exits 0, or timeout.
# Returns 0 on success, 1 on timeout.
poll_until() {
    local ms="$1"; shift
    local deadline=$(( $(date +%s%3N) + ms ))
    until "$@" 2>/dev/null; do
        [ "$(date +%s%3N)" -ge "$deadline" ] && return 1
        sleep 0.02
    done
}

# try_catch <catch_fn> <cmd...>
# Runs cmd. On non-zero exit, calls catch_fn with the exit code.
# Returns the cmd's exit code either way.
try_catch() {
    local catch_fn="$1"; shift
    if ! "$@"; then
        local code=$?
        "$catch_fn" "$code"
        return $code
    fi
}

# try_finally <finally_fn> <cmd...>
# Runs cmd, then ALWAYS runs finally_fn regardless of exit code.
try_finally() {
    local finally_fn="$1"; shift
    "$@"
    local code=$?
    "$finally_fn"
    return $code
}
