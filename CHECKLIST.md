# Checklists

## A) Tareas — hecho / pendiente

### ✅ Hecho
- [x] Alt+Tab: foco tras soltar Alt (libera grab de teclado antes de `focuswindow`)
- [x] Alt+Tab: **daemon instantáneo** `hypr-alttabd` (4 ms vs ~100 ms en frío)
- [x] Task View: miniaturas **distintas** por ventana
- [x] Task View: **miniaturas vivas reales** vía `hypr-winshot` (Rust + `hyprland_toplevel_export_v1`), sin parpadeo — equivalente a `DwmRegisterThumbnail`
- [x] Clipboard: fix del "VVVVV" en la inyección (release atómico + auto-cura de teclas atascadas)
- [x] Clipboard: imagen→ruta al pegar en terminal
- [x] Snap left/right/up/minimizar/restaurar (corregido `closespecialworkspace` inválido)
- [x] Reestructura a repo dotfiles + `install.sh` (symlinks, compila Rust)
- [x] Comparación con Windows + investigación (`COMPARISON.md`)
- [x] **Win+V: historial de portapapeles** (`hypr-clip-history`, cliphist+wofi) — *requiere instalar cliphist*
- [x] Push a GitHub

### ⏳ Pendiente / requiere acción tuya
- [ ] **Instalar cliphist** (AUR): `omarchy pkg aur add cliphist` — activa el Win+V
- [ ] Verificar interactivamente el clic derecho (ver tabla B)
- [ ] (Opcional) Reescritura COMPLETA en Rust de la UI del overlay/taskview (hoy solo la *captura* está en Rust; la UI sigue en Python/GTK)
- [ ] (Sugeridas) FancyZones/Snap Layouts, Aero Shake, Peek on-hover, Super+1..9

## B) Qué funciona bien / regular / mal

### 🟢 Bien — igual o MEJOR que Windows
| Función | Estado | Nota vs Windows |
|---|---|---|
| Alt+Tab (cambio) | ✅ 4 ms | **Igual** que el shell residente de Windows |
| Alt+Tab (overlay) | ✅ render correcto | Equivalente al switcher de twinui |
| Miniaturas vivas | ✅ vía Rust/toplevel-export | **Igual** a DWM (`DwmRegisterThumbnail`); mejor que nuestro método grim anterior |
| Snap mitades/maximizar | ✅ exacto | Igual a Win Snap |
| Minimizar/restaurar | ✅ (bug corregido) | Igual |
| Ctrl+C / Ctrl+V smart | ✅ probado sin VVVVV | Windows no tiene "Ctrl+C copia o SIGINT" |
| Imagen→ruta al pegar | ✅ | **Windows no lo tiene** |
| Portabilidad (git+install) | ✅ | **Mejor**: Windows usa registro opaco |

### 🟡 Regular — funciona pero con matices
| Función | Estado | Detalle |
|---|---|---|
| Clic derecho = pegar | 🟡 a verificar | Es Paste **nativo** de Alacritty (no inyecta). El "VVVVV" que viste es una **tecla V atascada** en el dispositivo virtual de ydotool (de pruebas/inyecciones previas), no del clic derecho en sí. Mitigado: cada inyección ahora libera teclas atascadas. **Probar de nuevo.** |
| Win+V historial | 🟡 listo, falta dep | Funciona en cuanto instales `cliphist` |
| Overlay arranque | 🟡 ~30-50 MB RAM | Python/GTK; un daemon 100% Rust bajaría a ~2-5 MB |

### 🔴 Mal / no implementado todavía
| Función | Estado |
|---|---|
| UI overlay/taskview en Rust | 🔴 sigue en Python/GTK (solo la captura es Rust) |
| FancyZones / Snap Layouts | 🔴 no implementado (sugerido) |
| Aero Shake / Peek / Timeline | 🔴 no implementado (sugerido) |

## Cómo se probó (reproducible)
- Overlay/TaskView: `ydotool` mantiene la tecla + `grim` captura → inspección visual.
- Cambio de ventana / snap: `hyprctl clients`/`activewindow` antes-después.
- Clipboard: terminal con `stdbuf -o0 cat > archivo`, pegar, comparar bytes exactos.
- `hypr-winshot`: capturar ventana ocluida por dirección → PNG válido con su contenido real.
