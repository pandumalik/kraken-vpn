#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::Path;

use eframe::egui;
use kraken_vpn::models::AppSettings;
use kraken_vpn::ovpn::parse_ovpn;
use kraken_vpn::platform::{check_binary, is_elevated};
use kraken_vpn::secrets::{protect_secret, unprotect_secret};
use kraken_vpn::store::ProfileStore;
use kraken_vpn::ui::KrakenApp;

const APP_ICON_PNG: &[u8] = include_bytes!("../icon.png");

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("kraken-vpn {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--check" || a == "--self-check") {
        std::process::exit(run_self_check());
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Kraken VPN")
        .with_inner_size([1100.0, 740.0])
        .with_min_inner_size([780.0, 540.0]);

    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Kraken VPN",
        options,
        Box::new(|cc| Ok(Box::new(KrakenApp::new(cc, APP_ICON_PNG)))),
    )
}

fn load_window_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(APP_ICON_PNG).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn run_self_check() -> i32 {
    println!("Kraken VPN v{} — Self-Check", env!("CARGO_PKG_VERSION"));
    println!("Elevated (Admin): {}", is_elevated());

    let store = match ProfileStore::default_os_store() {
        Ok(s) => {
            println!("Data directory:   {}", s.root_dir().display());
            s
        }
        Err(e) => {
            eprintln!("ERROR: store init failed: {e}");
            return 1;
        }
    };

    let settings = store
        .load_settings()
        .unwrap_or_else(|_| AppSettings::default());
    let ovpn_check = check_binary(&settings.openvpn_path);
    println!(
        "OpenVPN binary:   {} ({})",
        if ovpn_check.found {
            "FOUND"
        } else {
            "NOT FOUND"
        },
        ovpn_check
            .version_line
            .or(ovpn_check.error)
            .unwrap_or_else(|| settings.openvpn_path.clone())
    );

    let oc_check = check_binary(&settings.openconnect_path);
    println!(
        "OpenConnect bin:  {} ({})",
        if oc_check.found { "FOUND" } else { "NOT FOUND" },
        oc_check
            .version_line
            .or(oc_check.error)
            .unwrap_or_else(|| settings.openconnect_path.clone())
    );

    match protect_secret("kraken-self-check-probe")
        .and_then(|blob| unprotect_secret(blob.as_deref()))
    {
        Ok(val) if val == "kraken-self-check-probe" => {
            println!("Secret store:     OK (round-trip verified)");
        }
        Ok(_) => {
            eprintln!("ERROR: secret round-trip mismatch");
            return 1;
        }
        Err(e) => {
            eprintln!("ERROR: secret protection failed: {e}");
            return 1;
        }
    }

    match load_window_icon() {
        Some(icon) => println!("App icon:         OK ({}x{})", icon.width, icon.height),
        None => {
            eprintln!("ERROR: embedded icon.png failed to decode");
            return 1;
        }
    }

    let sample = Path::new("sample.ovpn");
    if sample.is_file() {
        if let Ok(content) = std::fs::read_to_string(sample) {
            let parsed = parse_ovpn(&content, "sample.ovpn");
            println!(
                "Sample .ovpn:     OK (remote={:?}:{}, proto={})",
                parsed.remote_host, parsed.remote_port, parsed.transport
            );
        }
    }

    println!("Self-check complete: OK");
    0
}
