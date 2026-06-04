# Comparación con Windows + checklist de pruebas

Cómo se comportan estas funciones "estilo Windows" en este sistema (Hyprland/Wayland)
frente a cómo las implementa Windows internamente, qué lenguajes usa cada uno, y
dónde podemos igualar o **superar** a Windows.

## Resultados de las pruebas (2026-06-04, Hyprland 0.55.2)

| # | Función | Cómo se probó | Resultado |
|---|---------|---------------|-----------|
| 1 | Alt+Tab — overlay | `ydotool` mantiene Alt + `grim` captura | ✅ renderiza la franja de tarjetas, 2ª seleccionada |
| 2 | Alt+Tab — cambio de ventana | cliente → daemon, medido | ✅ **4 ms** (antes ~100 ms en frío) |
| 3 | Alt+Shift+Tab (prev) | switch funcional | ✅ selecciona la última (MRU) |
| 4 | Task View (Super+Tab) | `grim` del overlay | ✅ miniaturas **distintas** por ventana |
| 5 | Snap left/right/up | geometría vía `hyprctl` | ✅ 683=mitad, 1366=completo |
| 6 | Minimizar/restaurar | workspace especial | ✅ (corregido `closespecialworkspace` inválido) |
| 7 | Clipboard pegar en terminal | volcado a archivo | ✅ **exacto, sin "VVVVV"** |
| 8 | Clipboard imagen→ruta | PNG + clipboard | ✅ guarda PNG y pega la ruta |
| 9 | window-list / screenshot | deps + listado | ✅ wofi/grim/slurp presentes |

## Comparación de implementación

| Aspecto | Windows | Este sistema |
|---|---|---|
| **Compositor** | DWM (`dwm.exe`) — C++ | Hyprland — C++ |
| **Alt+Tab UI** | `twinui.dll` / XAML (moderno), C++/C# | overlay GTK4 + layer-shell, Python |
| **Orden MRU** | lista Z-order de top-level del shell | `focusHistoryID` de Hyprland |
| **Apertura** | proceso residente del shell (instantáneo) | **daemon residente** `hypr-alttabd` (instantáneo) |
| **Miniaturas vivas** | `DwmRegisterThumbnail` (DWM dibuja la ventana viva en una región) | `grim` + `alterzorder` (snapshot por captura) |
| **Clipboard** | Win32 `SetClipboardData`/historial (`Win+V`) | `wl-clipboard` + inyección `ydotool` |
| **Lenguaje núcleo** | C++ (DWM), C#/XAML (shell) | C++ (Hyprland), Python/Bash (scripts) |

`dwmapi.dll` es una capa fina que hace LPC al proceso `dwm.exe`; nuestro cliente
`hypr-alttab` (socat) es análogo: una capa fina que habla por socket con el daemon.

## Dónde IGUALAMOS o SUPERAMOS a Windows

- **Apertura instantánea**: igual que el shell de Windows, el daemon mantiene el
  proceso caliente. Ya logrado (4 ms).
- **Transparencia/portabilidad**: todo es texto versionado en git con un instalador
  de symlinks → reproducible en otra máquina en segundos. Windows: configuración
  opaca en registro, no portable.
- **Personalización total**: comportamiento exacto (qué cuenta como terminal, qué
  hace Ctrl+C, snap a mitades) editable. Windows: limitado a lo que exponen.

## Dónde Windows nos GANA hoy (y cómo cerrarlo)

1. **Miniaturas VIVAS y sin parpadeo.** Windows usa `DwmRegisterThumbnail`: el
   compositor dibuja el contenido vivo de cada ventana en una región, sin tocar el
   z-order. Nosotros subimos la ventana con `alterzorder` y capturamos con `grim`
   (un breve parpadeo + imagen estática).
   **Cierre:** el protocolo `hyprland_toplevel_export_v1` (equivalente Wayland del
   thumbnail de DWM) permite capturar el framebuffer de una ventana **sin** subirla.
   No hay CLI para ello, pero sí buenas bindings en **Rust** (`wayland-client`).

2. **Latencia de arranque del overlay.** Python+GTK arranca caliente pero el
   proceso pesa ~30–50 MB RSS. Un daemon en **Rust** (p. ej. con
   `smithay-client-toolkit`) pesaría ~2–5 MB y pintaría aún más rápido.
   (De hecho `hyprswitch`, que ya tenías instalado, está escrito en Rust.)

## Optimización con Rust — propuesta

Reescribir `hypr-alttabd` + `hypr-taskview` como **un único daemon en Rust** que:
- Hable `wayland-client` + `hyprland_toplevel_export_v1` → **miniaturas vivas reales**
  (supera a nuestro método actual e iguala a DWM, sin el parpadeo de `alterzorder`).
- Use `layer-shell` nativo (sin LD_PRELOAD ni GTK).
- Exponga el mismo socket IPC (los bindings de Hyprland no cambian).
- Resultado: menor RAM, arranque sub-milisegundo, miniaturas vivas.

Coste: reimplementar UI en Rust (p. ej. `iced`/`smithay`). Beneficio alto si se
busca igualar la fluidez de Windows.

## Funciones de Windows que aún NO tenemos (sugerencias para implementar)

| Prioridad | Función Windows | Cómo hacerlo en Hyprland |
|---|---|---|
| Alta | **Historial de portapapeles (Win+V)** | `cliphist` + wofi/walker; bind a `SUPER, V` |
| Alta | **Snap Layouts / FancyZones** (cuadrículas) | extender `hypr-snap` con zonas (esquinas, tercios) o `hyprland` master/dwindle presets |
| Media | **Aero Shake** (minimizar las demás al sacudir) | detectar movimiento rápido del cursor + `hyprctl` |
| Media | **Peek / preview al pasar el ratón** en la barra | thumbnails on-hover en waybar vía toplevel-export |
| Media | **Win+números** = lanzar/enfocar app fija de la barra | binds `SUPER, 1..9` → `omarchy-launch-or-focus` |
| Baja | **Timeline / actividades recientes** | registro de ventanas+tiempos, búsqueda en walker |
| Baja | **Escritorios virtuales con miniaturas** (Task View completo) | extender `hypr-taskview` con fila de workspaces |
| Baja | **Snap assist** (sugerir ventana para la otra mitad) | tras snap, mostrar overlay con las demás ventanas |

## Fuentes

- [Desktop Window Manager (DWM) — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/_dwm/)
- [DwmRegisterThumbnail — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmregisterthumbnail)
- [APIs in the Desktop Window Manager — Microsoft (Greg Schechter)](https://learn.microsoft.com/en-us/archive/blogs/greg_schechter/apis-in-the-desktop-window-manager)
- [DwmRegisterThumbnail (Rust binding) — windows-docs-rs](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Graphics/Dwm/fn.DwmRegisterThumbnail.html)
- [Alt-Tab — Wikipedia](https://en.wikipedia.org/wiki/Alt-Tab)
