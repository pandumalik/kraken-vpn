# Kraken VPN (Pure Rust)

Kraken VPN is a lightweight, 100% pure Rust desktop VPN manager built with `egui`/`eframe`. It centralizes multiple **OpenVPN** and **OpenConnect** (`anyconnect`, `fortinet`, `gp`/GlobalProtect, `pulse`, `nc`/Juniper) profiles inside a single native Windows application while strictly enforcing **at most one active VPN tunnel at a time**.

---

## Installation (Windows)

Download and run the latest **`KrakenVPN-Setup-<version>-windows-x86_64.exe`** from the [GitHub Releases](../../releases) page:

- Installs Kraken VPN to `C:\Program Files\Kraken VPN\` with Start Menu / optional Desktop shortcuts and a standard Windows Uninstaller.
- Automatically detects whether **OpenVPN Community** and **OpenConnect-GUI** are already installed, and offers to download and install any missing VPN engines directly during setup.
- Verify downloads with the accompanying `SHA256SUMS.txt`.

---

## Key Features

- **Single-Active-VPN Mutual Exclusion**: Manage all your work and personal VPN profiles in one app. Connecting a second profile either cleanly disconnects the active tunnel first (**Auto-switch**) or blocks until you disconnect (**Strict**), with zero orphaned child processes.
- **Guaranteed Process & Temp-File Cleanup**: Uses a Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) plus process-tree termination on disconnect/exit, and shreds temporary OpenVPN auth/passphrase files on drop.
- **OpenVPN Support**: Import `.ovpn` files via native file dialog or point to a file path on disk, optional username/password auth and encrypted private-key/PKCS#12 passphrase (`--askpass`) written only to short-lived temp files (never logged), configurable `openvpn.exe` path, and OpenVPN management socket byte-count telemetry.
- **OpenConnect Support**: Protocol selection (`anyconnect`, `fortinet`, `gp`, `pulse`, `nc`), server URL/port, optional `--authgroup` and `--interface`, non-interactive password submission via `--passwd-on-stdin` (never in argv or logs), FortiGate DPD tuning (`--force-dpd=60`), `--servercert` pinning, non-blocking endpoint test, and explicit certificate-pin approval dialog.
- **Encrypted Local Storage**: Profiles, settings, and connection history are stored in `%APPDATA%\KrakenVPN\` (or OS config dir). Stored passwords and key passphrases are encrypted at rest via **Windows DPAPI** (`CryptProtectData`, current-user scope).
- **Live Monitoring & Terminal Logs**:
  - Dedicated **Monitoring** tab with real-time download/upload throughput graph, peak speeds, and session totals.
  - **Logs** tab featuring a high-contrast **Live Terminal** monitor (line numbers, millisecond timestamps, color-coded `SYS`/`INFO`/`WARN`/`ERR` rows, live filter counts, auto-scroll, search, Copy/Clear) alongside persistent **Connection History**.
- **Customizable Themes & Color Palettes**: Switch between **Light** and **Dark** modes, pick from 6 built-in presets (*Forest Sage*, *Ocean Slate*, *Nord Frost*, *Midnight Violet*, *Amber Carbon*, *Monochrome*), or customize individual UI colors per mode with one-click reset.
- **Windows Elevation Awareness**: Detects Administrator status via the Win32 `TokenElevation` API and provides a one-click **Relaunch as Admin** banner when non-elevated.

---

## Prerequisites

Kraken VPN orchestrates the native VPN CLI engines installed on your machine (and offers to install them during setup or via the **Download** button in **Settings**):

1. **OpenVPN** — auto-detected at `C:\Program Files\OpenVPN\bin\openvpn.exe` or on `PATH` (configurable in Settings).
2. **OpenConnect** — auto-detected at `C:\Program Files\OpenConnect-GUI\openconnect.exe` or on `PATH` (with `wintun.dll` / `vpnc-script-win.js` on Windows, configurable in Settings).

---

## Building & Running

```powershell
# Check formatting
cargo fmt --all -- --check

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

# Build Windows installer locally (requires Inno Setup 6: winget install JRSoftware.InnoSetup)
.\installer\build-installer.ps1
```

---

## CI/CD & Releases

- **[`.github/workflows/ci.yml`](./.github/workflows/ci.yml)**: Runs formatting, Clippy, unit/integration tests, Windows release smoke tests, security scans (Gitleaks + `cargo-audit`), and uploads a test Windows installer artifact on pushes to `main`/`master` and PRs.
- **[`.github/workflows/build.yml`](./.github/workflows/build.yml)**: Triggers on `v*` tags (matching `Cargo.toml` version) or manual workflow dispatch to build, verify, and publish `KrakenVPN-Setup-<version>-windows-x86_64.exe` and `SHA256SUMS.txt` to GitHub Releases.

---

## Architecture

```text
kraken-vpn/
├── Cargo.toml
├── icon-2.png
├── sample.ovpn
├── installer/
│   ├── kraken-vpn.iss        # Inno Setup 6 script (auto-detects & downloads VPN engines)
│   └── build-installer.ps1   # Local & CI helper to generate .ico and compile Setup .exe
├── src/
│   ├── main.rs               # CLI flags (--version, --check), icon loader, UAC elevation, eframe bootstrap
│   ├── lib.rs                # Library exports
│   ├── models.rs             # Profiles, protocols, theme presets/colors, settings, history
│   ├── secrets.rs            # Windows DPAPI credential encryption/decryption
│   ├── store.rs              # Atomic JSON persistence in %APPDATA%\KrakenVPN\
│   ├── ovpn.rs               # .ovpn directive parser & profile importer
│   ├── platform.rs           # Win32 Admin check, UAC relaunch, Job Object guard, window/binary helpers
│   ├── telemetry.rs          # sysinfo::Networks live speed/bytes tracker & formatters
│   ├── tray.rs               # Windows system tray icon & menu controller
│   ├── vpn/
│   │   ├── mod.rs            # VPN module exports
│   │   ├── state.rs          # Mutually exclusive single-active-VPN state machine
│   │   ├── openvpn.rs        # OpenVPN args, RAII shredding temp files, stdout parser
│   │   ├── openconnect.rs    # OpenConnect args, cert-pin detector, endpoint tester
│   │   └── manager.rs        # Thread-safe process orchestrator & log streaming
│   └── ui/
│       ├── mod.rs            # KrakenApp shell, navigation rail, delete & cert-pin modals
│       ├── theme.rs          # Material 3 palette presets, custom color derivation, UI primitives
│       ├── dashboard.rs      # Active tunnel hero card, quick-connect, saved profiles table
│       ├── monitoring.rs     # Live bandwidth chart, throughput peaks, session telemetry
│       ├── profiles.rs       # Responsive Add/Edit profile modal, .ovpn importer, endpoint test
│       ├── diagnostics.rs    # Terminal-style live log monitor & connection history table
│       └── settings.rs       # Switch policy, CLI paths (Browse/Test/Download), color palette
└── tests/
    └── vpn_workflows.rs      # Integration tests (fixtures, redaction, temp cleanup, policies)
```

---

## License

Open-sourced under the **Kraken VPN Non-Commercial License** — see [`LICENSE`](./LICENSE).
