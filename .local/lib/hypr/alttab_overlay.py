#!/usr/bin/env python3
"""Overlay del Alt+Tab para Hyprland (módulo, usado por el daemon hypr-alttabd).

Antes era un script de un solo uso (alt-tab.py) que llamaba a Gtk.Application.quit
al confirmar. Ahora es una VENTANA reutilizable: el daemon la crea por invocación
y la destruye al cerrar, pero la Gtk.Application del daemon sigue viva (imports de
GTK ya calientes → apertura instantánea, sin arranque en frío).

Flujo:
  - El daemon recibe "next"/"prev" por socket → crea OverlayWindow y show_overlay()
  - Teclado EXCLUSIVE: detecta Tab/Shift+Tab para ciclar y Alt release para cerrar
  - El bindr `ALT, Alt_L` en Hyprland envía "close" como respaldo
  - Al confirmar: libera el grab de teclado ANTES de enfocar (si no, Hyprland
    ignora `focuswindow` mientras una capa tiene el teclado en exclusiva)
"""

import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
gi.require_version('Gtk4LayerShell', '1.0')
from gi.repository import Gtk, Gdk, GLib, Gtk4LayerShell
import subprocess, json, os

# ── Helpers de Hyprland ─────────────────────────────────────────────────────────

def get_clients_mru():
    r = subprocess.run(['hyprctl', 'clients', '-j'], capture_output=True, text=True)
    if r.returncode != 0:
        return []
    clients = [
        c for c in json.loads(r.stdout)
        if c.get('workspace', {}).get('id', -1) > 0
        or 'special:minimized' in c.get('workspace', {}).get('name', '')
    ]
    return sorted(clients, key=lambda c: c.get('focusHistoryID', 999))


def _restore_and_focus(addr: str):
    r = subprocess.run(['hyprctl', 'activeworkspace', '-j'], capture_output=True, text=True)
    ws_id = json.loads(r.stdout).get('id', 1)
    # IPC separado: movetoworkspace desde workspace especial tiene procesamiento async
    subprocess.run(['hyprctl', 'dispatch', 'movetoworkspace', f'{ws_id},address:{addr}'],
                   capture_output=True)
    win = json.loads(subprocess.run(
        ['hyprctl', 'activewindow', '-j'], capture_output=True, text=True).stdout)
    if win.get('floating', False):
        subprocess.run(['hyprctl', 'dispatch', 'bringactivetotop'], capture_output=True)
    else:
        # Ventana tileada: no tiene Z-order, snap up para que cubra las demás
        subprocess.run(['hypr-snap', 'up'], capture_output=True)


def focus_window(addr: str):
    subprocess.run(['hyprctl', '--batch',
        f'dispatch focuswindow address:{addr} ; dispatch bringactivetotop'],
        capture_output=True)
    # Si la ventana enfocada es TILEADA, las flotantes maximizadas la taparían:
    # en Hyprland las flotantes se dibujan por encima de las tileadas y
    # bringactivetotop no cambia eso. La subimos al frente con snap up (flujo
    # estilo Windows: todo es flotante-maximizado), igual que _restore_and_focus.
    win = json.loads(subprocess.run(
        ['hyprctl', 'activewindow', '-j'], capture_output=True, text=True).stdout or '{}')
    if not win.get('floating', False):
        subprocess.run(['hypr-snap', 'up'], capture_output=True)


def get_icon_image(class_name: str) -> Gtk.Image:
    theme = Gtk.IconTheme.get_for_display(Gdk.Display.get_default())
    candidates = dict.fromkeys([
        class_name.lower(),
        class_name,
        class_name.split('.')[-1].lower(),
        class_name.split('-')[0].lower(),
    ])
    name = next((n for n in candidates if theme.has_icon(n)), 'application-x-executable')
    return Gtk.Image.new_from_icon_name(name)


# ── CSS (instalado una vez por el daemon) ───────────────────────────────────────

CSS = b"""
window {
    background-color: rgba(13, 14, 24, 0.94);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 16px;
    padding: 18px 24px;
}
.card {
    background-color: rgba(36, 40, 59, 0.88);
    border: 2px solid rgba(255,255,255,0.10);
    border-radius: 10px;
    padding: 12px 10px 8px 10px;
    margin: 0px 4px;
}
.card.selected {
    border-color: rgba(255,255,255,0.88);
    background-color: rgba(122,162,247,0.10);
}
.card-title {
    font-size: 11px;
    color: #a9b1d6;
    margin-top: 6px;
}
"""

_css_installed = False

def install_css():
    """Instala el provider CSS una sola vez (es global al display)."""
    global _css_installed
    if _css_installed:
        return
    provider = Gtk.CssProvider()
    provider.load_from_data(CSS)
    Gtk.StyleContext.add_provider_for_display(
        Gdk.Display.get_default(), provider,
        Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    _css_installed = True


# ── Ventana del overlay ─────────────────────────────────────────────────────────

class OverlayWindow(Gtk.ApplicationWindow):
    def __init__(self, app, initial_direction='next', on_closed=None):
        super().__init__(application=app)
        self.on_closed = on_closed
        self.initial_direction = initial_direction
        self.clients = []
        self.selected = 0
        self.cards = []
        self._closing = False
        self.set_decorated(False)
        self.set_resizable(False)

    # ── Mostrar ──────────────────────────────────────────────────────────────────

    def show_overlay(self):
        self.clients = get_clients_mru()
        if not self.clients:
            self._finish()
            return
        n = len(self.clients)
        self.selected = (n - 1) if self.initial_direction == 'prev' else (1 if n > 1 else 0)

        self._setup_layer_shell()
        self.set_child(self._build_card_strip())

        key_ctrl = Gtk.EventControllerKey()
        key_ctrl.connect('key-pressed', self._on_key_pressed)
        key_ctrl.connect('key-released', self._on_key_released)
        self.add_controller(key_ctrl)

        self.present()
        self.cards[self.selected].add_css_class('selected')
        # Respaldo para tap rápido: si Alt ya no está al abrir, confirmar.
        GLib.timeout_add(70, self._check_alt_held)

    def _setup_layer_shell(self):
        Gtk4LayerShell.init_for_window(self)
        Gtk4LayerShell.set_namespace(self, 'hypr-alttab')
        Gtk4LayerShell.set_layer(self, Gtk4LayerShell.Layer.OVERLAY)
        Gtk4LayerShell.set_anchor(self, Gtk4LayerShell.Edge.BOTTOM, True)
        Gtk4LayerShell.set_margin(self, Gtk4LayerShell.Edge.BOTTOM, 40)
        Gtk4LayerShell.set_keyboard_mode(self, Gtk4LayerShell.KeyboardMode.EXCLUSIVE)

    def _build_card_strip(self) -> Gtk.Box:
        strip = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=0)
        strip.set_halign(Gtk.Align.CENTER)
        self.cards = [self._build_card(c) for c in self.clients]
        for card in self.cards:
            strip.append(card)
        return strip

    def _build_card(self, client: dict) -> Gtk.Box:
        card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        card.add_css_class('card')
        card.set_valign(Gtk.Align.CENTER)
        card.set_size_request(130, -1)

        icon = get_icon_image(client.get('class', ''))
        icon.set_pixel_size(52)
        icon.set_halign(Gtk.Align.CENTER)
        card.append(icon)

        raw = client.get('title', '') or client.get('class', 'Unknown')
        short = raw[:18] + '…' if len(raw) > 18 else raw
        lbl = Gtk.Label(label=short)
        lbl.add_css_class('card-title')
        lbl.set_halign(Gtk.Align.CENTER)
        lbl.set_max_width_chars(16)
        card.append(lbl)

        return card

    # ── Navegación / teclado ─────────────────────────────────────────────────────

    def cycle(self, delta: int):
        if not self.cards:
            return
        self.cards[self.selected].remove_css_class('selected')
        self.selected = (self.selected + delta) % len(self.clients)
        self.cards[self.selected].add_css_class('selected')

    def _check_alt_held(self):
        seat = Gdk.Display.get_default().get_default_seat()
        kbd = seat.get_keyboard()
        if kbd is not None and not (kbd.get_modifier_state() & Gdk.ModifierType.ALT_MASK):
            self.confirm_and_close()
        return GLib.SOURCE_REMOVE

    def _on_key_pressed(self, ctrl, keyval, keycode, state):
        if keyval == Gdk.KEY_Tab:
            self.cycle(1)
            return True
        if keyval == Gdk.KEY_ISO_Left_Tab:  # Shift+Tab
            self.cycle(-1)
            return True
        if keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            self.confirm_and_close()
            return True
        if keyval == Gdk.KEY_Escape:
            self.cancel()
            return True
        return False

    def _on_key_released(self, ctrl, keyval, keycode, state):
        if keyval in (Gdk.KEY_Alt_L, Gdk.KEY_Alt_R):
            self.confirm_and_close()
            return True
        return False

    # ── Cierre ───────────────────────────────────────────────────────────────────

    def confirm_and_close(self):
        # Puede invocarse varias veces (Alt release + bindr 'close' + _check_alt_held).
        if self._closing:
            return
        self._closing = True

        target = None
        if self.clients and 0 <= self.selected < len(self.clients):
            target = self.clients[self.selected]

        # CLAVE: liberar el grab de teclado del layer-shell ANTES de enfocar.
        # Con KeyboardMode.EXCLUSIVE, Hyprland IGNORA `dispatch focuswindow`.
        Gtk4LayerShell.set_keyboard_mode(self, Gtk4LayerShell.KeyboardMode.NONE)
        self.set_visible(False)

        def _do_focus():
            if target:
                addr = target.get('address', '')
                if addr:
                    if 'special:minimized' in target.get('workspace', {}).get('name', ''):
                        _restore_and_focus(addr)
                    else:
                        focus_window(addr)
            self._finish()
            return False

        GLib.timeout_add(25, _do_focus)

    def cancel(self):
        if self._closing:
            return
        self._closing = True
        Gtk4LayerShell.set_keyboard_mode(self, Gtk4LayerShell.KeyboardMode.NONE)
        self._finish()

    def _finish(self):
        cb = self.on_closed
        self.destroy()
        if cb:
            cb()
