#!/usr/bin/env bash
# clip-lib.sh — Librería compartida para los scripts de portapapeles.
# Hacer source, no ejecutar.

source ~/.local/lib/bash/async.sh

export YDOTOOL_SOCKET=/run/user/$(id -u)/.ydotool_socket

TERMINALS="Alacritty kitty foot wezterm ghostty"

is_terminal() {
    local class="$1"
    for t in $TERMINALS; do [ "$class" = "$t" ] && return 0; done
    return 1
}

active_window_class() {
    hyprctl activewindow -j 2>/dev/null \
        | python3 -c "import json,sys; print(json.load(sys.stdin).get('class',''))" 2>/dev/null
}

# Inyecta teclas saltando los bindings de Hyprland via submap passthrough.
# IMPORTANTE: hyprctl dispatch submap devuelve exit 0 aunque el submap no exista —
# no se puede usar el exit code para verificar. Se comprueba el texto de salida.
#
# Fix del bug "VVVVV": la tecla inyectada se quedaba en autorepeat (40/s tras
# 250ms) cuando su evento de "soltar" se perdía o cuando el `submap reset` se
# ejecutaba mientras los eventos inyectados seguían en vuelo (re-disparando el
# bind real). Mitigaciones:
#   1) --key-delay separa los eventos para que no se coalescan/pierdan.
#   2) Red de seguridad: soltamos explícitamente TODAS las teclas que pudimos
#      usar (Ctrl/Shift/C/V) — un "up" sobre una tecla no presionada es inocuo.
#   3) Settle ANTES del reset: los eventos inyectados drenan estando aún en el
#      submap passthrough, así no re-disparan el bind ni quedan huérfanos.
inject_key_bypassed() {
    local result
    result=$(hyprctl dispatch submap clipboard-passthrough 2>&1)
    if echo "$result" | grep -qiE "wasn't registered|doesn't exist"; then
        echo "inject_key_bypassed: submap clipboard-passthrough no registrado" >&2
        hyprctl dispatch submap reset
        return 1
    fi
    sleep 0.01                              # asegurar submap activo antes de inyectar
    ydotool key --key-delay 6 "$@"
    ydotool key --key-delay 1 29:0 42:0 46:0 47:0   # red de seguridad: soltar todo
    sleep 0.03                              # drenar eventos inyectados antes del reset
    hyprctl dispatch submap reset
}

# Espera hasta que el clipboard cambie respecto al md5 dado.
# Usa poll_until (de async.sh) en lugar de sleep fijo.
# $1: md5 inicial   $2: timeout en ms (default 150)
wait_clipboard_change() {
    local before="$1" timeout_ms="${2:-150}"
    poll_until "$timeout_ms" bash -c \
        "[ \"$(wl-paste --no-newline 2>/dev/null | md5sum)\" != \"$before\" ]"
}
