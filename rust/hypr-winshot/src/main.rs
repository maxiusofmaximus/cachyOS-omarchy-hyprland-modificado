//! hypr-winshot — Captura el framebuffer VIVO de una ventana de Hyprland a PNG,
//! usando el protocolo `hyprland_toplevel_export_v1` (equivalente Wayland del
//! `DwmRegisterThumbnail` de Windows). A diferencia de `grim`, NO captura una
//! región de pantalla: captura el contenido real de la ventana aunque esté
//! ocluida, sin tocar el z-order ni provocar parpadeo.
//!
//! Uso:  hypr-winshot <window-address> <out.png>
//!   p.ej.  hypr-winshot 0x558cf8b33b60 /tmp/win.png
//!
//! El handle del protocolo es uint (32 bits); Hyprland compara los 32 bits bajos
//! de la dirección de la ventana, así que pasamos `address & 0xFFFFFFFF`.

use std::os::fd::AsFd;
use wayland_client::{
    protocol::{wl_buffer, wl_registry, wl_shm, wl_shm_pool},
    Connection, Dispatch, Proxy, QueueHandle,
};

// ── Bindings del protocolo (generados desde el XML en tiempo de compilación) ──
mod toplevel_export {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-toplevel-export-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/hyprland-toplevel-export-v1.xml");
}

use toplevel_export::hyprland_toplevel_export_frame_v1::{
    self, HyprlandToplevelExportFrameV1,
};
use toplevel_export::hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1;

#[derive(Clone, Copy)]
struct BufParams {
    format: u32,
    width: u32,
    height: u32,
    stride: u32,
}

struct State {
    shm: Option<wl_shm::WlShm>,
    manager: Option<HyprlandToplevelExportManagerV1>,
    params: Option<BufParams>,
    buffer_done: bool,
    y_invert: bool,
    ready: bool,
    failed: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("uso: hypr-winshot <window-address> <out.png>");
        std::process::exit(2);
    }
    let addr_str = args[1].trim_start_matches("0x");
    let addr = u64::from_str_radix(addr_str, 16).unwrap_or_else(|_| {
        eprintln!("dirección inválida: {}", args[1]);
        std::process::exit(2);
    });
    let handle = (addr & 0xFFFF_FFFF) as u32;
    let out_path = &args[2];

    let conn = Connection::connect_to_env().expect("no se pudo conectar a Wayland");
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let display = conn.display();
    display.get_registry(&qh, ());

    let mut state = State {
        shm: None,
        manager: None,
        params: None,
        buffer_done: false,
        y_invert: false,
        ready: false,
        failed: false,
    };

    // Resolver globals (wl_shm + el manager del toplevel-export).
    queue.roundtrip(&mut state).unwrap();
    let shm = state.shm.clone().expect("falta wl_shm");
    let manager = state
        .manager
        .clone()
        .expect("el compositor no expone hyprland_toplevel_export_manager_v1");

    // Pedir la captura de la ventana por handle.
    let _frame = manager.capture_toplevel(0, handle, &qh, ());

    // Esperar los parámetros del buffer (buffer/buffer_done).
    while state.params.is_none() && !state.failed {
        queue.blocking_dispatch(&mut state).unwrap();
    }
    while !state.buffer_done && !state.failed {
        queue.blocking_dispatch(&mut state).unwrap();
    }
    if state.failed {
        eprintln!("captura fallida (ventana inexistente o protocolo rechazó)");
        std::process::exit(1);
    }
    let p = state.params.expect("sin parámetros de buffer");

    // Crear el buffer shm.
    let size = (p.stride * p.height) as usize;
    let file = tempfile::tempfile().expect("tmpfile");
    file.set_len(size as u64).unwrap();
    let pool = shm.create_pool(file.as_fd(), size as i32, &qh, ());
    let buffer = pool.create_buffer(
        0,
        p.width as i32,
        p.height as i32,
        p.stride as i32,
        wl_shm::Format::try_from(p.format).unwrap_or(wl_shm::Format::Xrgb8888),
        &qh,
        (),
    );

    // Copiar el contenido de la ventana al buffer.
    _frame.copy(&buffer, 0);
    while !state.ready && !state.failed {
        queue.blocking_dispatch(&mut state).unwrap();
    }
    if state.failed {
        eprintln!("copy falló");
        std::process::exit(1);
    }

    // Leer el shm y convertir a RGBA.
    let mmap = unsafe { memmap2::MmapOptions::new().len(size).map(&file).unwrap() };
    let mut rgba = vec![0u8; (p.width * p.height * 4) as usize];
    let is_bgr = matches!(p.format, 0 | 1 | 875708993 | 875709016); // ARGB/XRGB8888 → BGRA en memoria
    for y in 0..p.height {
        let src_y = if state.y_invert { p.height - 1 - y } else { y };
        for x in 0..p.width {
            let si = (src_y * p.stride + x * 4) as usize;
            let di = ((y * p.width + x) * 4) as usize;
            if si + 3 < mmap.len() {
                let (b0, b1, b2, b3) = (mmap[si], mmap[si + 1], mmap[si + 2], mmap[si + 3]);
                if is_bgr {
                    rgba[di] = b2;     // R
                    rgba[di + 1] = b1; // G
                    rgba[di + 2] = b0; // B
                } else {
                    rgba[di] = b0;
                    rgba[di + 1] = b1;
                    rgba[di + 2] = b2;
                }
                rgba[di + 3] = 255;    // opaco (ignorar alpha real)
            }
        }
    }

    image::save_buffer(
        out_path,
        &rgba,
        p.width,
        p.height,
        image::ColorType::Rgba8,
    )
    .expect("no se pudo escribir el PNG");
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, version.min(1), qh, ()));
                }
                "hyprland_toplevel_export_manager_v1" => {
                    state.manager = Some(registry.bind(name, version.min(2), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<HyprlandToplevelExportManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &HyprlandToplevelExportManagerV1,
        _: <HyprlandToplevelExportManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<HyprlandToplevelExportFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &HyprlandToplevelExportFrameV1,
        event: hyprland_toplevel_export_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use hyprland_toplevel_export_frame_v1::Event;
        match event {
            Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                // Tomar el primer formato shm ofrecido.
                if state.params.is_none() {
                    let fmt = match format {
                        wayland_client::WEnum::Value(f) => f as u32,
                        wayland_client::WEnum::Unknown(f) => f,
                    };
                    state.params = Some(BufParams {
                        format: fmt,
                        width,
                        height,
                        stride,
                    });
                }
            }
            Event::BufferDone => state.buffer_done = true,
            Event::Flags { flags } => {
                let f = match flags {
                    wayland_client::WEnum::Value(v) => v.bits(),
                    wayland_client::WEnum::Unknown(v) => v,
                };
                state.y_invert = f & 1 != 0;
            }
            Event::Ready { .. } => state.ready = true,
            Event::Failed => state.failed = true,
            _ => {}
        }
    }
}

impl Dispatch<wl_shm::WlShm, ()> for State {
    fn event(_: &mut Self, _: &wl_shm::WlShm, _: wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_shm_pool::WlShmPool, ()> for State {
    fn event(_: &mut Self, _: &wl_shm_pool::WlShmPool, _: wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(_: &mut Self, _: &wl_buffer::WlBuffer, _: wl_buffer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
