use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::{SwitchPolicy, VpnProtocol};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TunnelState {
    #[default]
    Disconnected,
    Connecting {
        profile_id: Uuid,
        profile_name: String,
        protocol: VpnProtocol,
        endpoint: String,
        started_at: DateTime<Utc>,
    },
    Connected {
        profile_id: Uuid,
        profile_name: String,
        protocol: VpnProtocol,
        endpoint: String,
        connected_at: DateTime<Utc>,
    },
    Disconnecting {
        profile_id: Uuid,
        profile_name: String,
    },
    Error {
        profile_id: Uuid,
        profile_name: String,
        message: String,
    },
}

impl TunnelState {
    pub fn active_profile_id(&self) -> Option<Uuid> {
        match self {
            TunnelState::Connecting { profile_id, .. }
            | TunnelState::Connected { profile_id, .. }
            | TunnelState::Disconnecting { profile_id, .. } => Some(*profile_id),
            TunnelState::Disconnected | TunnelState::Error { .. } => None,
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, TunnelState::Connected { .. })
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            TunnelState::Connecting { .. }
                | TunnelState::Connected { .. }
                | TunnelState::Disconnecting { .. }
        )
    }

    pub fn connected_duration_secs(&self, now: DateTime<Utc>) -> u64 {
        match self {
            TunnelState::Connected { connected_at, .. } => now
                .signed_duration_since(*connected_at)
                .num_seconds()
                .max(0) as u64,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectDecision {
    StartNow,

    AlreadyActive,

    DisconnectThenSwitch {
        active_profile_id: Uuid,
        active_profile_name: String,
    },

    Refused {
        active_profile_name: String,
    },
}

pub fn decide_connect(
    current: &TunnelState,
    target_profile_id: Uuid,
    policy: SwitchPolicy,
) -> ConnectDecision {
    match current {
        TunnelState::Disconnected | TunnelState::Error { .. } => ConnectDecision::StartNow,
        TunnelState::Connecting {
            profile_id,
            profile_name,
            ..
        }
        | TunnelState::Connected {
            profile_id,
            profile_name,
            ..
        } => {
            if *profile_id == target_profile_id {
                ConnectDecision::AlreadyActive
            } else {
                match policy {
                    SwitchPolicy::AutoSwitch => ConnectDecision::DisconnectThenSwitch {
                        active_profile_id: *profile_id,
                        active_profile_name: profile_name.clone(),
                    },
                    SwitchPolicy::RefuseWhileActive => ConnectDecision::Refused {
                        active_profile_name: profile_name.clone(),
                    },
                }
            }
        }
        TunnelState::Disconnecting { profile_name, .. } => ConnectDecision::Refused {
            active_profile_name: profile_name.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_single_active_vpn_auto_switch_and_refusal() {
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();

        assert_eq!(
            decide_connect(&TunnelState::Disconnected, p1, SwitchPolicy::AutoSwitch),
            ConnectDecision::StartNow
        );

        let active = TunnelState::Connected {
            profile_id: p1,
            profile_name: "Work Fortinet".into(),
            protocol: VpnProtocol::OpenConnect,
            endpoint: "vpn.work.example:443".into(),
            connected_at: Utc::now(),
        };

        assert_eq!(
            decide_connect(&active, p1, SwitchPolicy::AutoSwitch),
            ConnectDecision::AlreadyActive
        );
        assert_eq!(
            decide_connect(&active, p2, SwitchPolicy::AutoSwitch),
            ConnectDecision::DisconnectThenSwitch {
                active_profile_id: p1,
                active_profile_name: "Work Fortinet".into(),
            }
        );
        assert_eq!(
            decide_connect(&active, p2, SwitchPolicy::RefuseWhileActive),
            ConnectDecision::Refused {
                active_profile_name: "Work Fortinet".into(),
            }
        );

        let err_state = TunnelState::Error {
            profile_id: p1,
            profile_name: "Work Fortinet".into(),
            message: "AUTH_FAILED".into(),
        };
        assert_eq!(
            decide_connect(&err_state, p2, SwitchPolicy::RefuseWhileActive),
            ConnectDecision::StartNow
        );
    }

    #[test]
    fn refuses_new_connection_while_disconnecting() {
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let state = TunnelState::Disconnecting {
            profile_id: p1,
            profile_name: "Home OpenVPN".into(),
        };
        assert_eq!(
            decide_connect(&state, p2, SwitchPolicy::AutoSwitch),
            ConnectDecision::Refused {
                active_profile_name: "Home OpenVPN".into(),
            }
        );
    }
}
