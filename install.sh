#!/usr/bin/env bash
# install.sh — Enlaza (symlink) los dotfiles de este repo dentro de $HOME.
#
# Estructura: el repo refleja la jerarquía de $HOME. Cada archivo bajo
# .config/ y .local/ se enlaza a su ruta equivalente en $HOME. Los archivos
# reales que ya existan se respaldan con sufijo .bak.<timestamp> antes de enlazar.
#
# Idempotente: re-ejecutarlo enlaza archivos nuevos y deja los ya correctos.
#
# Uso:
#   ./install.sh           # enlaza todo
#   ./install.sh --dry-run # muestra qué haría, sin tocar nada

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DRY=0
[ "${1:-}" = "--dry-run" ] && DRY=1

STAMP="$(date +%Y%m%d-%H%M%S)"
linked=0 backed=0 skipped=0

log() { printf '%s\n' "$*"; }

link_file() {
    local src="$1" dst="$2"
    # Ya enlazado correctamente → nada que hacer
    if [ -L "$dst" ] && [ "$(readlink -f "$dst")" = "$(readlink -f "$src")" ]; then
        skipped=$((skipped+1)); return
    fi
    if [ "$DRY" = 1 ]; then
        [ -e "$dst" ] && log "BACKUP  $dst -> $dst.bak.$STAMP"
        log "LINK    $dst -> $src"
        return
    fi
    mkdir -p "$(dirname "$dst")"
    if [ -e "$dst" ] || [ -L "$dst" ]; then
        mv "$dst" "$dst.bak.$STAMP"
        backed=$((backed+1))
        log "backup:  $dst -> $dst.bak.$STAMP"
    fi
    ln -s "$src" "$dst"
    linked=$((linked+1))
    log "link:    $dst -> $src"
}

# Recorre todos los archivos versionados bajo .config y .local
while IFS= read -r -d '' src; do
    rel="${src#"$REPO"/}"
    link_file "$src" "$HOME/$rel"
done < <(find "$REPO/.config" "$REPO/.local" -type f -print0 2>/dev/null)

# Asegurar permisos de ejecución en los comandos
if [ "$DRY" != 1 ]; then
    chmod +x "$REPO"/.local/bin/* 2>/dev/null || true
fi

# Compilar y enlazar las herramientas en Rust (hypr-winshot: miniaturas vivas).
# Si no hay cargo, se omite y hypr-taskview cae automáticamente a grim+alterzorder.
WINSHOT_SRC="$REPO/rust/hypr-winshot"
WINSHOT_BIN="$WINSHOT_SRC/target/release/hypr-winshot"
if [ -d "$WINSHOT_SRC" ]; then
    if [ "$DRY" = 1 ]; then
        log "BUILD   cargo build --release ($WINSHOT_SRC) y LINK ~/.local/bin/hypr-winshot"
    elif command -v cargo >/dev/null 2>&1; then
        log ""
        log "Compilando hypr-winshot (Rust)…"
        ( cd "$WINSHOT_SRC" && cargo build --release ) \
            && ln -sf "$WINSHOT_BIN" "$HOME/.local/bin/hypr-winshot" \
            && log "link:    $HOME/.local/bin/hypr-winshot -> $WINSHOT_BIN" \
            || log "aviso:   falló la compilación de hypr-winshot (taskview usará grim)"
    else
        log "aviso:   cargo no encontrado — omito hypr-winshot (taskview usará grim)"
    fi
fi

log ""
log "Hecho. Enlazados: $linked · respaldados: $backed · sin cambios: $skipped"
log ""
log "Siguientes pasos:"
log "  1) hyprctl reload                 # recargar bindings"
log "  2) env LD_PRELOAD=/usr/lib/libgtk4-layer-shell.so hypr-alttabd &  # o relogin"
log "  3) Verifica: hyprctl configerrors"
