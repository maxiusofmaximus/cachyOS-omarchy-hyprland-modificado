# dotfiles — Hyprland (Omarchy) personalizado

Personalizaciones estilo Windows para Hyprland sobre Omarchy: Alt+Tab con overlay,
Task View con miniaturas reales, snapping de ventanas, y portapapeles inteligente.
Pensado para portarse entre máquinas (este PC y el servidor del homelab).

## Estructura

```
.config/hypr/
  bindings.conf      # keybindings (llaman a los comandos hypr-*)
  autostart.conf     # lanza ydotoold + el daemon hypr-alttabd
.local/bin/          # comandos (en PATH del usuario)
  hypr-alttabd       # daemon residente del Alt+Tab (overlay instantáneo)
  hypr-alttab        # cliente rápido (socat) → next|prev|close|quit
  hypr-taskview      # Super+Tab: vista de tareas con miniaturas reales
  hypr-snap          # Super+flechas: snap/maximizar/minimizar
  hypr-window-list   # Super+Shift+Tab: lista de ventanas (wofi)
  hypr-screenshot    # Super+Shift+S: captura de región
  hypr-clip-copy     # Ctrl+C inteligente (copia o SIGINT)
  hypr-clip-paste    # Ctrl+V inteligente (imagen → ruta en terminal)
.local/lib/
  bash/async.sh      # primitivas async para bash
  hypr/clip-lib.sh   # librería del portapapeles (inyección con ydotool)
  hypr/alttab_overlay.py  # UI del overlay (importada por el daemon)
```

## Instalación (en este PC o en el otro)

```bash
git clone <repo-url> ~/dotfiles
cd ~/dotfiles
./install.sh           # crea symlinks (respalda lo existente como .bak.<fecha>)
hyprctl reload
```

El daemon arranca solo en el siguiente login (autostart.conf). Para arrancarlo ya:

```bash
env LD_PRELOAD=/usr/lib/libgtk4-layer-shell.so hypr-alttabd &
```

`./install.sh --dry-run` muestra qué haría sin tocar nada.

## Dependencias

`hyprland`, `python-gobject`, `gtk4`, `gtk4-layer-shell`, `grim`, `socat`,
`ydotool` (+ `ydotoold`), `wl-clipboard`, `wofi`, y los comandos `omarchy-*`.

## Notas de diseño

- **Alt+Tab instantáneo**: un daemon mantiene Python+GTK calientes; el cliente
  solo envía un comando por socket UNIX. Antes cada pulsación arrancaba Python
  en frío (~50-100ms).
- **Foco tras soltar Alt**: el overlay usa una capa con teclado `EXCLUSIVE`;
  Hyprland ignora `focuswindow` mientras esa capa tiene el grab, así que se
  libera el teclado (modo `NONE`) ANTES de enfocar.
- **Miniaturas reales (Task View)**: `grim` captura regiones de pantalla, no
  framebuffers por ventana. Las flotantes ocluidas se suben con `alterzorder`
  (sin cambiar el foco), se capturan, y se restaura el orden.
- **Portapapeles**: Ctrl+C/V se interceptan y se reinyecta Ctrl+Shift+C/V con
  ydotool dentro de un submap passthrough; se sueltan las teclas explícitamente
  y se deja drenar antes del reset para evitar autorepeat de la tecla inyectada.
