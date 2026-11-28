use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HistoryStatus {
    Connected,
    Disconnected,
    Failed,
}

impl HistoryStatus {
    pub fn label(&self) -> &'static str {
        match self {
            HistoryStatus::Connected => "Connected",
            HistoryStatus::Disconnected => "Disconnected",
            HistoryStatus::Failed => "Failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionHistoryEntry {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub profile_name: String,
    pub protocol: String,
    pub endpoint: String,
    pub status: HistoryStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: u64,
    pub message: String,
}
