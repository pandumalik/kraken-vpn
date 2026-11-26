# Kraken VPN (Pure Rust)

Kraken VPN is a lightweight, 100% pure Rust desktop VPN manager built with `egui`/`eframe`. It centralizes multiple **OpenVPN** and **OpenConnect** (`anyconnect`, `fortinet`, `gp`/GlobalProtect, `pulse`, `nc`/Juniper) profiles inside a single native executable while strictly enforcing **at most one active VPN tunnel at a time**.

> The original Electron JS implementation is preserved for reference under [`legacy-electron/`](./legacy-electron/).

---

## Key Features

- **Single-Active-VPN Mutual Exclusion**: Manage all your work and personal VPN profiles in one app. Connecting a second profile either cleanly disconnects the active tunnel first (**Auto-switch**) or blocks until you disconnect (**Strict**), with zero orphaned child processes.
- **Guaranteed Process & Temp-File Cleanup**: Uses a Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) plus process-tree termination on disconnect/exit, and shreds temporary OpenVPN auth files on drop.
- **OpenVPN Support**: Import `.ovpn` files via native file dialog or point to a file path on disk, optional username/password auth (written only to short-lived temp files, never logged), configurable `openvpn.exe` path, and OpenVPN management socket byte-count telemetry.
- **OpenConnect Support**: Protocol selection (`anyconnect`, `fortinet`, `gp`, `pulse`, `nc`), server URL/port, optional `--authgroup` and `--interface`, password submission via `stdin` (never in argv or logs), `--servercert` pinning, non-blocking endpoint test, and explicit certificate-pin approval dialog.
- **Encrypted Local Storage**: Profiles, settings, and connection history are stored in `%APPDATA%\KrakenVPN\` (or OS config dir). Stored passwords are encrypted at rest via **Windows DPAPI** (`CryptProtectData`, current-user scope).
- **Live Telemetry & Diagnostics**: Real-time auto-scaling download/upload speeds and session totals with a live sparkline, searchable timestamped stdout/stderr Diagnostics Console (with Copy/Clear), and persistent Connection History.
- **Windows Elevation Awareness**: Detects Administrator status via the Win32 `TokenElevation` API and provides a one-click **Relaunch as Admin** banner when non-elevated.

---

## Prerequisites

Kraken VPN orchestrates the native CLI engines installed on your machine:

1. **OpenVPN** — default path `C:\Program Files\OpenVPN\bin\openvpn.exe` (configurable in Settings).
2. **OpenConnect** — `openconnect.exe` on `PATH` or configured directly in Settings (with `wintun` / `vpnc-script-win.js` on Windows).

---

## Building & Running

```powershell
# Run all unit & integration tests
cargo test

# Lint with Clippy (warnings denied)
cargo clippy --all-targets -- -D warnings

# Build optimized release binary (target/release/kraken-vpn.exe)
cargo build --release

# Headless smoke test (version, DPAPI round-trip, icon decode, binary detection)
.\target\release\kraken-vpn.exe --version
.\target\release\kraken-vpn.exe --check

# Launch desktop GUI
cargo run --release
```

---

## Architecture

```text
kraken-vpn/
├── Cargo.toml
├── icon.png
├── sample.ovpn
├── legacy-electron/        # Original Electron JS version (reference)
├── src/
│   ├── main.rs             # CLI flags (--version, --check), icon loader, eframe bootstrap
│   ├── lib.rs              # Library exports
│   ├── models.rs           # Profiles, protocols, settings, history, diagnostics models
│   ├── secrets.rs          # Windows DPAPI credential encryption/decryption
│   ├── store.rs            # Atomic JSON persistence in %APPDATA%\KrakenVPN\
│   ├── ovpn.rs             # .ovpn directive parser & profile importer
│   ├── platform.rs         # Win32 Admin check, UAC relaunch, Job Object guard, binary resolver
│   ├── telemetry.rs        # sysinfo::Networks live speed/bytes tracker & formatters
│   ├── vpn/
│   │   ├── state.rs        # Mutually exclusive single-active-VPN state machine
│   │   ├── openvpn.rs      # OpenVPN args, RAII Shredding temp files, stdout parser
│   │   ├── openconnect.rs  # OpenConnect args, cert-pin detector, endpoint tester
│   │   └── manager.rs      # Thread-safe process orchestrator & log streaming
│   └── ui/
│       ├── mod.rs          # KrakenApp shell, sidebar, Dark/Light themes, cert-pin modal
│       ├── dashboard.rs    # Active tunnel hero card, live telemetry sparkline, quick-connect
│       ├── profiles.rs     # Profile CRUD, .ovpn importer, OpenConnect tester
│       ├── diagnostics.rs  # Live stdout/stderr console & connection history table
│       └── settings.rs     # Binary paths (Browse/Test), switch policy, theme
└── tests/
    └── vpn_workflows.rs    # Integration tests (fixtures, redaction, temp cleanup, policies)
```

---

## License

Open-sourced under the **Kraken VPN Non-Commercial License** — see [`LICENSE`](./LICENSE).