//! hypr-fancyzones — FancyZones / Snap Layouts para Hyprland.
//!
//! Define "zonas" (rectángulos en fracciones [0..1] del área usable del monitor)
//! agrupadas en "layouts", y hace snap de la ventana activa a una zona — como las
//! Snap Layouts / FancyZones de Windows, pero por teclado.
//!
//! Config:  ~/.config/hypr/fancyzones.json   (se crea con defaults si falta)
//! Estado:  ~/.cache/hypr/fancyzones-current (layout actual)
//!
//! Uso:
//!   hypr-fancyzones snap <N>       # snap de la ventana activa a la zona N (1-based)
//!   hypr-fancyzones layout <name>  # fijar el layout actual
//!   hypr-fancyzones cycle          # siguiente layout
//!   hypr-fancyzones list           # listar layouts y zonas del actual
//!
//! Bindings sugeridos:
//!   bind = SUPER CTRL, 1, exec, hypr-fancyzones snap 1   (…2..6)
//!   bind = SUPER CTRL, TAB, exec, hypr-fancyzones cycle

use serde_json::Value;
use std::process::Command;

const DEFAULT_CONFIG: &str = r#"{
  "default": "halves",
  "layouts": {
    "halves":    [[0.0,0.0,0.5,1.0],[0.5,0.0,0.5,1.0]],
    "thirds":    [[0.0,0.0,0.3333,1.0],[0.3333,0.0,0.3334,1.0],[0.6667,0.0,0.3333,1.0]],
    "main-side": [[0.0,0.0,0.6667,1.0],[0.6667,0.0,0.3333,0.5],[0.6667,0.5,0.3333,0.5]],
    "quad":      [[0.0,0.0,0.5,0.5],[0.5,0.0,0.5,0.5],[0.0,0.5,0.5,0.5],[0.5,0.5,0.5,0.5]],
    "grid6":     [[0.0,0.0,0.3333,0.5],[0.3333,0.0,0.3334,0.5],[0.6667,0.0,0.3333,0.5],[0.0,0.5,0.3333,0.5],[0.3333,0.5,0.3334,0.5],[0.6667,0.5,0.3333,0.5]]
  }
}"#;

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/root".into())
}
fn config_path() -> String {
    format!("{}/.config/hypr/fancyzones.json", home())
}
fn state_path() -> String {
    format!("{}/.cache/hypr/fancyzones-current", home())
}

fn hyprctl_json(args: &[&str]) -> Value {
    let out = Command::new("hyprctl").args(args).arg("-j").output();
    match out {
        Ok(o) if o.status.success() => {
            serde_json::from_slice(&o.stdout).unwrap_or(Value::Null)
        }
        _ => Value::Null,
    }
}

fn hyprctl_dispatch(batch: &str) {
    let _ = Command::new("hyprctl").arg("--batch").arg(batch).output();
}

fn load_config() -> Value {
    let p = config_path();
    if let Ok(s) = std::fs::read_to_string(&p) {
        if let Ok(v) = serde_json::from_str::<Value>(&s) {
            return v;
        }
    }
    // crear defaults
    if let Some(dir) = std::path::Path::new(&p).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&p, DEFAULT_CONFIG);
    serde_json::from_str(DEFAULT_CONFIG).unwrap()
}

fn layout_names(cfg: &Value) -> Vec<String> {
    let mut names: Vec<String> = cfg
        .get("layouts")
        .and_then(|l| l.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    names
}

fn current_layout(cfg: &Value) -> String {
    if let Ok(s) = std::fs::read_to_string(state_path()) {
        let name = s.trim().to_string();
        if cfg.pointer(&format!("/layouts/{}", name)).is_some() {
            return name;
        }
    }
    cfg.get("default")
        .and_then(|d| d.as_str())
        .map(String::from)
        .unwrap_or_else(|| layout_names(cfg).first().cloned().unwrap_or_default())
}

fn set_current_layout(name: &str) {
    let p = state_path();
    if let Some(dir) = std::path::Path::new(&p).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&p, name);
}

/// Área usable del monitor enfocado, en coordenadas LÓGICAS: (x, y, w, h).
fn usable_area() -> Option<(i64, i64, i64, i64)> {
    let mons = hyprctl_json(&["monitors"]);
    let m = mons.as_array()?.iter().find(|m| {
        m.get("focused").and_then(|f| f.as_bool()).unwrap_or(false)
    })?;
    let scale = m.get("scale").and_then(|s| s.as_f64()).unwrap_or(1.0);
    let pw = m.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let ph = m.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mx = m.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
    let my = m.get("y").and_then(|v| v.as_i64()).unwrap_or(0);
    // reserved = [left, top, right, bottom] en lógico
    let r = m.get("reserved").and_then(|v| v.as_array());
    let res = |i: usize| -> i64 {
        r.and_then(|a| a.get(i)).and_then(|v| v.as_i64()).unwrap_or(0)
    };
    let lw = (pw / scale).round() as i64;
    let lh = (ph / scale).round() as i64;
    let x = mx + res(0);
    let y = my + res(1);
    let w = lw - res(0) - res(2);
    let h = lh - res(1) - res(3);
    Some((x, y, w, h))
}

fn active_window() -> Option<(String, bool)> {
    let w = hyprctl_json(&["activewindow"]);
    let addr = w.get("address").and_then(|a| a.as_str())?.to_string();
    if addr.is_empty() {
        return None;
    }
    let floating = w.get("floating").and_then(|f| f.as_bool()).unwrap_or(false);
    Some((addr, floating))
}

fn snap_to_zone(cfg: &Value, zone_1based: usize) {
    let layout = current_layout(cfg);
    let zones = match cfg.pointer(&format!("/layouts/{}", layout)).and_then(|z| z.as_array()) {
        Some(z) => z,
        None => {
            eprintln!("layout '{}' sin zonas", layout);
            return;
        }
    };
    if zone_1based == 0 || zone_1based > zones.len() {
        eprintln!("zona {} fuera de rango (1..{})", zone_1based, zones.len());
        return;
    }
    let z = zones[zone_1based - 1].as_array().unwrap();
    let f = |i: usize| z.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let (ux, uy, uw, uh) = match usable_area() {
        Some(a) => a,
        None => {
            eprintln!("no se pudo leer el monitor");
            return;
        }
    };
    let zx = ux + (f(0) * uw as f64).round() as i64;
    let zy = uy + (f(1) * uh as f64).round() as i64;
    let zw = (f(2) * uw as f64).round() as i64;
    let zh = (f(3) * uh as f64).round() as i64;

    let (addr, floating) = match active_window() {
        Some(a) => a,
        None => {
            eprintln!("sin ventana activa");
            return;
        }
    };
    if !floating {
        // FancyZones trabaja con ventanas flotantes (como el snap estilo Windows).
        let _ = Command::new("hyprctl")
            .args(["dispatch", "togglefloating", &format!("address:{}", addr)])
            .output();
    }
    hyprctl_dispatch(&format!(
        "dispatch resizewindowpixel exact {} {},address:{} ; dispatch movewindowpixel exact {} {},address:{}",
        zw, zh, addr, zx, zy, addr
    ));
}

fn cmd_cycle(cfg: &Value) {
    let names = layout_names(cfg);
    if names.is_empty() {
        return;
    }
    let cur = current_layout(cfg);
    let idx = names.iter().position(|n| *n == cur).unwrap_or(0);
    let next = &names[(idx + 1) % names.len()];
    set_current_layout(next);
    notify(&format!("Layout: {}", next));
    println!("{}", next);
}

fn notify(msg: &str) {
    let _ = Command::new("notify-send")
        .args(["-t", "1200", "FancyZones", msg])
        .output();
}

fn main() {
    let cfg = load_config();
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("list");
    match cmd {
        "snap" => {
            let n: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            snap_to_zone(&cfg, n);
        }
        "layout" => {
            if let Some(name) = args.get(2) {
                if cfg.pointer(&format!("/layouts/{}", name)).is_some() {
                    set_current_layout(name);
                    notify(&format!("Layout: {}", name));
                } else {
                    eprintln!("layout desconocido: {}", name);
                }
            }
        }
        "cycle" => cmd_cycle(&cfg),
        "list" => {
            let cur = current_layout(&cfg);
            println!("layout actual: {}", cur);
            println!("disponibles: {}", layout_names(&cfg).join(", "));
            if let Some(z) = cfg.pointer(&format!("/layouts/{}", cur)).and_then(|z| z.as_array()) {
                for (i, zone) in z.iter().enumerate() {
                    println!("  zona {}: {}", i + 1, zone);
                }
            }
        }
        other => eprintln!("comando desconocido: {} (usa: snap N | layout NAME | cycle | list)", other),
    }
}
