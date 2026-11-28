use std::collections::VecDeque;
use std::time::Instant;

use sysinfo::Networks;

const HISTORY_SAMPLES: usize = 60;

#[derive(Debug, Clone, Default)]
pub struct TelemetrySnapshot {
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
    pub peak_rx_bytes_per_sec: f64,
    pub peak_tx_bytes_per_sec: f64,
    pub session_rx_bytes: u64,
    pub session_tx_bytes: u64,
    pub active_interface: Option<String>,
    pub rx_history: Vec<f64>,
    pub tx_history: Vec<f64>,
}

pub struct TelemetryTracker {
    networks: Networks,
    last_sample_at: Option<Instant>,
    rx_history: VecDeque<f64>,
    tx_history: VecDeque<f64>,
    peak_rx: f64,
    peak_tx: f64,
    session_rx: u64,
    session_tx: u64,
    preferred_interface: Option<String>,

    override_totals: Option<(u64, u64)>,
    last_override_totals: Option<(u64, u64)>,
    override_baseline: (u64, u64),
    list_refresh_ticks: u8,
}

impl Default for TelemetryTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryTracker {
    pub fn new() -> Self {
        Self {
            networks: Networks::new_with_refreshed_list(),
            last_sample_at: None,
            rx_history: VecDeque::with_capacity(HISTORY_SAMPLES),
            tx_history: VecDeque::with_capacity(HISTORY_SAMPLES),
            peak_rx: 0.0,
            peak_tx: 0.0,
            session_rx: 0,
            session_tx: 0,
            preferred_interface: None,
            override_totals: None,
            last_override_totals: None,
            override_baseline: (0, 0),
            list_refresh_ticks: 0,
        }
    }

    pub fn reset_session(&mut self, preferred_interface: Option<String>) {
        self.networks.refresh_list();
        self.last_sample_at = Some(Instant::now());
        self.rx_history.clear();
        self.tx_history.clear();
        self.peak_rx = 0.0;
        self.peak_tx = 0.0;
        self.session_rx = 0;
        self.session_tx = 0;
        self.preferred_interface = preferred_interface;
        self.override_totals = None;
        self.last_override_totals = None;
        self.override_baseline = (0, 0);
        self.list_refresh_ticks = 0;
    }

    pub fn reset_counters(&mut self) {
        self.rx_history.clear();
        self.tx_history.clear();
        self.peak_rx = 0.0;
        self.peak_tx = 0.0;
        self.session_rx = 0;
        self.session_tx = 0;
        if let Some(cur) = self.override_totals {
            self.override_baseline = cur;
            self.last_override_totals = Some(cur);
        }
    }

    pub fn update_tunnel_bytecount(&mut self, bytes_in: u64, bytes_out: u64) {
        self.override_totals = Some((bytes_in, bytes_out));
    }

    pub fn sample(&mut self, connected: bool) -> TelemetrySnapshot {
        if !connected {
            return TelemetrySnapshot::default();
        }

        let now = Instant::now();
        let elapsed = self
            .last_sample_at
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(0.0);
        self.last_sample_at = Some(now);

        let (rx_rate, tx_rate, iface) = if let Some((cur_in, cur_out)) = self.override_totals {
            let (d_in, d_out) = match self.last_override_totals {
                Some((prev_in, prev_out)) => (
                    cur_in.saturating_sub(prev_in),
                    cur_out.saturating_sub(prev_out),
                ),
                None => (0, 0),
            };
            self.last_override_totals = Some((cur_in, cur_out));
            self.session_rx = cur_in.saturating_sub(self.override_baseline.0);
            self.session_tx = cur_out.saturating_sub(self.override_baseline.1);
            let rx_s = if elapsed > 0.05 {
                d_in as f64 / elapsed
            } else {
                0.0
            };
            let tx_s = if elapsed > 0.05 {
                d_out as f64 / elapsed
            } else {
                0.0
            };
            (rx_s, tx_s, Some("OpenVPN Tunnel".to_string()))
        } else {
            if self.list_refresh_ticks < 5 {
                self.networks.refresh_list();
                self.list_refresh_ticks += 1;
            } else {
                self.networks.refresh();
            }
            let (d_rx, d_tx, iface_name) = self.read_network_delta();
            self.session_rx = self.session_rx.saturating_add(d_rx);
            self.session_tx = self.session_tx.saturating_add(d_tx);
            let rx_s = if elapsed > 0.05 {
                d_rx as f64 / elapsed
            } else {
                0.0
            };
            let tx_s = if elapsed > 0.05 {
                d_tx as f64 / elapsed
            } else {
                0.0
            };
            (rx_s, tx_s, iface_name)
        };

        if rx_rate > self.peak_rx {
            self.peak_rx = rx_rate;
        }
        if tx_rate > self.peak_tx {
            self.peak_tx = tx_rate;
        }

        if self.rx_history.len() >= HISTORY_SAMPLES {
            self.rx_history.pop_front();
        }
        if self.tx_history.len() >= HISTORY_SAMPLES {
            self.tx_history.pop_front();
        }
        self.rx_history.push_back(rx_rate);
        self.tx_history.push_back(tx_rate);

        TelemetrySnapshot {
            rx_bytes_per_sec: rx_rate,
            tx_bytes_per_sec: tx_rate,
            peak_rx_bytes_per_sec: self.peak_rx,
            peak_tx_bytes_per_sec: self.peak_tx,
            session_rx_bytes: self.session_rx,
            session_tx_bytes: self.session_tx,
            active_interface: iface,
            rx_history: self.rx_history.iter().copied().collect(),
            tx_history: self.tx_history.iter().copied().collect(),
        }
    }

    fn read_network_delta(&self) -> (u64, u64, Option<String>) {
        if let Some(ref pref) = self.preferred_interface {
            let pref_lower = pref.to_ascii_lowercase();
            for (name, data) in &self.networks {
                if name.to_ascii_lowercase().contains(&pref_lower) {
                    return (data.received(), data.transmitted(), Some(name.clone()));
                }
            }
        }

        let mut vpn_rx = 0u64;
        let mut vpn_tx = 0u64;
        let mut vpn_name: Option<String> = None;
        for (name, data) in &self.networks {
            if is_vpn_interface_name(name) {
                vpn_rx = vpn_rx.saturating_add(data.received());
                vpn_tx = vpn_tx.saturating_add(data.transmitted());
                if vpn_name.is_none() {
                    vpn_name = Some(name.clone());
                }
            }
        }
        if vpn_name.is_some() {
            return (vpn_rx, vpn_tx, vpn_name);
        }

        let mut total_rx = 0u64;
        let mut total_tx = 0u64;
        for (name, data) in &self.networks {
            let lower = name.to_ascii_lowercase();
            if lower == "lo" || lower.contains("loopback") {
                continue;
            }
            total_rx = total_rx.saturating_add(data.received());
            total_tx = total_tx.saturating_add(data.transmitted());
        }
        (total_rx, total_tx, Some("System Network".to_string()))
    }
}

pub fn is_vpn_interface_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("wintun")
        || lower.contains("openvpn")
        || lower.contains("tap-windows")
        || lower.contains("openconnect")
        || lower.starts_with("tun")
        || lower.starts_with("utun")
        || lower.starts_with("tap")
}

pub fn parse_openvpn_bytecount(line: &str) -> Option<(u64, u64)> {
    let rest = line.trim().strip_prefix(">BYTECOUNT:")?;
    let (in_s, out_s) = rest.split_once(',')?;
    let bytes_in = in_s.trim().parse::<u64>().ok()?;
    let bytes_out = out_s.trim().parse::<u64>().ok()?;
    Some((bytes_in, bytes_out))
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut val = bytes as f64;
    let mut idx = 0;
    while val >= 1024.0 && idx < UNITS.len() - 1 {
        val /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} B")
    } else {
        format!("{val:.2} {}", UNITS[idx])
    }
}

pub fn format_speed(bytes_per_sec: f64) -> String {
    if !bytes_per_sec.is_finite() || bytes_per_sec <= 0.0 {
        return "0 B/s".to_string();
    }
    const UNITS: &[&str] = &["B/s", "KB/s", "MB/s", "GB/s"];
    let mut val = bytes_per_sec;
    let mut idx = 0;
    while val >= 1024.0 && idx < UNITS.len() - 1 {
        val /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{:.0} {}", val, UNITS[idx])
    } else {
        format!("{val:.2} {}", UNITS[idx])
    }
}

pub fn format_duration(total_secs: u64) -> String {
    let hrs = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    format!("{hrs:02}:{mins:02}:{secs:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_speeds_and_durations() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024 * 5), "5.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.00 GB");

        assert_eq!(format_speed(0.0), "0 B/s");
        assert_eq!(format_speed(-5.0), "0 B/s");
        assert_eq!(format_speed(f64::NAN), "0 B/s");
        assert_eq!(format_speed(500.0), "500 B/s");
        assert_eq!(format_speed(2048.0), "2.00 KB/s");
        assert_eq!(format_speed(1024.0 * 1024.0 * 12.5), "12.50 MB/s");

        assert_eq!(format_duration(0), "00:00:00");
        assert_eq!(format_duration(65), "00:01:05");
        assert_eq!(format_duration(3661), "01:01:01");
    }

    #[test]
    fn parses_openvpn_bytecount_lines() {
        assert_eq!(
            parse_openvpn_bytecount(">BYTECOUNT:12345,67890"),
            Some((12345, 67890))
        );
        assert_eq!(
            parse_openvpn_bytecount("  >BYTECOUNT:0,42\r\n"),
            Some((0, 42))
        );
        assert_eq!(parse_openvpn_bytecount("INFO: something"), None);
        assert_eq!(parse_openvpn_bytecount(">BYTECOUNT:bad,1"), None);
    }

    #[test]
    fn detects_vpn_interface_names() {
        assert!(is_vpn_interface_name("OpenVPN Wintun"));
        assert!(is_vpn_interface_name("TAP-Windows Adapter V9"));
        assert!(is_vpn_interface_name("tun0"));
        assert!(!is_vpn_interface_name("Ethernet"));
        assert!(!is_vpn_interface_name("Wi-Fi"));
    }

    #[test]
    fn tracks_peaks_and_resets_counters() {
        let mut tracker = TelemetryTracker::new();
        tracker.reset_session(None);
        tracker.update_tunnel_bytecount(10_000, 5_000);
        let first = tracker.sample(true);
        assert_eq!(first.session_rx_bytes, 10_000);
        assert_eq!(first.session_tx_bytes, 5_000);

        tracker.reset_counters();
        tracker.update_tunnel_bytecount(12_000, 5_500);
        let after = tracker.sample(true);
        assert_eq!(after.session_rx_bytes, 2_000);
        assert_eq!(after.session_tx_bytes, 500);
        assert!(after.peak_rx_bytes_per_sec >= 0.0);
        assert!(after.peak_tx_bytes_per_sec >= 0.0);
    }
}
