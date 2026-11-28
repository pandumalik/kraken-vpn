use std::fs;
use std::path::Path;

use kraken_vpn::models::{AppSettings, OpenConnectProtocol, SwitchPolicy, VpnProfile};
use kraken_vpn::ovpn::{parse_ovpn, profile_from_ovpn};
use kraken_vpn::store::ProfileStore;
use kraken_vpn::vpn::manager::VpnManager;
use kraken_vpn::vpn::openvpn::prepare_openvpn;
use kraken_vpn::vpn::state::{decide_connect, ConnectDecision, TunnelState};
use uuid::Uuid;

#[test]
fn parses_repo_sample_ovpn_fixture() {
    let sample_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("sample.ovpn");
    let content = fs::read_to_string(&sample_path).expect("sample.ovpn should exist");
    let parsed = parse_ovpn(&content, "sample.ovpn");

    assert_eq!(parsed.remote_host.as_deref(), Some("uk-lon.krakenvpn.net"));
    assert_eq!(parsed.remote_port, 1194);
    assert_eq!(parsed.transport, "UDP");
    assert!(parsed.requires_user_pass);

    let profile = profile_from_ovpn(Some(&sample_path), "sample.ovpn", &content).unwrap();
    assert_eq!(profile.name, "Sample");
    assert_eq!(profile.endpoint_display(), "uk-lon.krakenvpn.net:1194");
}

#[test]
fn profile_debug_redacts_password() {
    let mut profile =
        VpnProfile::new_openconnect("Prod", "vpn.example.com", OpenConnectProtocol::Pulse);
    profile.password = "NeverLogThisPassword123!".into();
    let dbg = format!("{profile:?}");
    assert!(!dbg.contains("NeverLogThisPassword123!"));
    assert!(dbg.contains("[REDACTED]"));
}

#[test]
fn openvpn_prepare_writes_and_cleans_temp_auth_and_config_files() {
    let temp_dir = std::env::temp_dir().join(format!("kraken_prep_test_{}", Uuid::new_v4()));
    let mut profile = VpnProfile::new_openvpn("Test OVPN");
    profile.ovpn_content =
        Some("client\ndev tun\nremote vpn.test.local 1194\nauth-user-pass\n".into());
    profile.username = "testuser".into();
    profile.password = "testpass".into();

    let (cfg_path, auth_path) = {
        let prepared = prepare_openvpn(&profile, &temp_dir).unwrap();
        let cfg_idx = prepared.args.iter().position(|a| a == "--config").unwrap();
        let auth_idx = prepared
            .args
            .iter()
            .position(|a| a == "--auth-user-pass")
            .unwrap();
        let cfg_p = std::path::PathBuf::from(&prepared.args[cfg_idx + 1]);
        let auth_p = std::path::PathBuf::from(&prepared.args[auth_idx + 1]);
        assert!(cfg_p.is_file());
        assert!(auth_p.is_file());
        assert_eq!(fs::read_to_string(&auth_p).unwrap(), "testuser\ntestpass\n");
        (cfg_p, auth_p)
    };

    assert!(!cfg_path.exists());
    assert!(!auth_path.exists());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn manager_missing_binary_returns_clean_error_without_orphans() {
    let temp_dir = std::env::temp_dir().join(format!("kraken_mgr_test_{}", Uuid::new_v4()));
    let mgr = VpnManager::new(temp_dir.clone());
    let settings = AppSettings {
        openvpn_path: r"Z:\nonexistent\openvpn_missing.exe".into(),
        openconnect_path: r"Z:\nonexistent\openconnect_missing.exe".into(),
        ..AppSettings::default()
    };

    let mut ovpn = VpnProfile::new_openvpn("Missing OVPN");
    ovpn.ovpn_content = Some("client\nremote 127.0.0.1 1194\n".into());
    assert!(mgr.connect(ovpn, &settings).is_err());
    assert_eq!(mgr.state(), TunnelState::Disconnected);

    let oc = VpnProfile::new_openconnect(
        "Missing OC",
        "vpn.example.com",
        OpenConnectProtocol::AnyConnect,
    );
    assert!(mgr.connect(oc, &settings).is_err());
    assert_eq!(mgr.state(), TunnelState::Disconnected);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn multi_profile_store_and_switch_policy_integration() {
    let dir = std::env::temp_dir().join(format!("kraken_multi_test_{}", Uuid::new_v4()));
    let store = ProfileStore::in_dir(&dir).unwrap();

    let mut p1 = VpnProfile::new_openvpn("Home Wire");
    p1.ovpn_content = Some("client\nremote home.example 1194\n".into());
    p1.username = "pandu".into();
    p1.password = "home-secret".into();

    let mut p2 = VpnProfile::new_openconnect(
        "Office Fortinet",
        "vpn.office.example",
        OpenConnectProtocol::Fortinet,
    );
    p2.port = Some(8443);
    p2.username = "corp-user".into();
    p2.password = "corp-secret".into();

    store.save_profiles(&[p1.clone(), p2.clone()]).unwrap();
    let reloaded = store.load_profiles().unwrap();
    assert_eq!(reloaded.len(), 2);
    assert_eq!(reloaded[0].password, "home-secret");
    assert_eq!(reloaded[1].password, "corp-secret");

    let connected_p1 = TunnelState::Connected {
        profile_id: p1.id,
        profile_name: p1.name.clone(),
        protocol: p1.protocol,
        endpoint: p1.endpoint_display(),
        connected_at: chrono::Utc::now(),
    };
    assert!(matches!(
        decide_connect(&connected_p1, p2.id, SwitchPolicy::AutoSwitch),
        ConnectDecision::DisconnectThenSwitch { .. }
    ));
    assert!(matches!(
        decide_connect(&connected_p1, p2.id, SwitchPolicy::RefuseWhileActive),
        ConnectDecision::Refused { .. }
    ));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fast_failing_openvpn_captures_full_stdout_and_root_cause_in_history() {
    let settings = AppSettings::default();
    if kraken_vpn::platform::resolve_binary(&settings.openvpn_path).is_none() {
        return;
    }

    let sample_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("sample.ovpn");
    let content = fs::read_to_string(&sample_path).expect("sample.ovpn should exist");
    let mut profile = profile_from_ovpn(Some(&sample_path), "sample.ovpn", &content).unwrap();
    profile.username = "probe-user".into();
    profile.password = "probe-pass".into();

    let temp_dir = std::env::temp_dir().join(format!("kraken_ovpn_err_test_{}", Uuid::new_v4()));
    let mgr = VpnManager::new(temp_dir.clone());
    mgr.connect(profile, &settings)
        .expect("spawn should succeed");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let mut history = Vec::new();
    while std::time::Instant::now() < deadline {
        history = mgr.take_pending_history();
        if !history.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    assert!(!history.is_empty(), "expected failed history entry");
    let detail = &history[0].message;
    assert!(
        detail.contains("Options error:") || detail.contains("CA"),
        "history should contain real OpenVPN root-cause error, got: {detail}"
    );

    let logs = mgr.diagnostics_snapshot();
    assert!(
        logs.iter()
            .any(|l| l.source == "OpenVPN" && l.message.contains("Options error:")),
        "live logs must preserve OpenVPN stdout error lines even on immediate exit; got: {:?}",
        logs.iter().map(|l| &l.message).collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
