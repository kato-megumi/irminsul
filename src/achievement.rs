use std::collections::BTreeMap;
use std::fmt::Display;

use anyhow::Result;
pub use auto_artifactarium::Achievement;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AchievementExportFormat {
    #[default]
    Uiaf,
    Seelie,
}

impl Display for AchievementExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AchievementExportFormat::Uiaf => write!(f, "UIAF v1.1"),
            AchievementExportFormat::Seelie => write!(f, "Seelie.me"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AchievementExportSettings {
    pub format: AchievementExportFormat,
    pub completed_only: bool,
}

impl Default for AchievementExportSettings {
    fn default() -> Self {
        Self {
            format: AchievementExportFormat::Uiaf,
            completed_only: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UiafInfo {
    pub export_app: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_app_version: Option<String>,
    pub uiaf_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_timestamp: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UiafItem {
    pub id: u32,
    pub timestamp: u64,
    pub current: u32,
    pub status: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Uiaf {
    pub info: UiafInfo,
    pub list: Vec<UiafItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SeelieItem {
    pub done: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SeelieExport {
    pub achievements: BTreeMap<String, SeelieItem>,
}

pub fn is_achievement_completed(status: u32) -> bool {
    status == 2 || status == 3
}

pub fn export_achievements(
    achievements: &[Achievement],
    settings: &AchievementExportSettings,
) -> Result<String> {
    match settings.format {
        AchievementExportFormat::Uiaf => export_uiaf(achievements, settings),
        AchievementExportFormat::Seelie => export_seelie(achievements, settings),
    }
}

pub fn export_uiaf(
    achievements: &[Achievement],
    settings: &AchievementExportSettings,
) -> Result<String> {
    let export_timestamp = chrono::Utc::now().timestamp() as u64;
    export_uiaf_with_timestamp(achievements, settings, export_timestamp)
}

pub fn export_uiaf_with_timestamp(
    achievements: &[Achievement],
    settings: &AchievementExportSettings,
    export_timestamp: u64,
) -> Result<String> {
    let mut list = Vec::new();

    for achievement in achievements {
        let is_completed = is_achievement_completed(achievement.status);
        if settings.completed_only && !is_completed {
            continue;
        }

        let timestamp = match achievement.finish_timestamp {
            Some(ts) => ts as u64,
            None => {
                if is_completed {
                    export_timestamp
                } else {
                    0
                }
            }
        };

        list.push(UiafItem {
            id: achievement.id,
            timestamp,
            current: 0,
            status: achievement.status,
        });
    }

    list.sort_by_key(|a| a.id);

    let uiaf = Uiaf {
        info: UiafInfo {
            export_app: "Irminsul".to_string(),
            export_app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            uiaf_version: "v1.1".to_string(),
            export_timestamp: Some(export_timestamp),
        },
        list,
    };

    let json = serde_json::to_string(&uiaf)?;
    tracing::trace!("{json}");
    Ok(json)
}

pub fn export_seelie(
    achievements: &[Achievement],
    settings: &AchievementExportSettings,
) -> Result<String> {
    let mut achievements_map = BTreeMap::new();

    for achievement in achievements {
        let is_completed = is_achievement_completed(achievement.status);
        if settings.completed_only && !is_completed {
            continue;
        }

        achievements_map.insert(
            achievement.id.to_string(),
            SeelieItem { done: is_completed },
        );
    }

    let seelie_export = SeelieExport {
        achievements: achievements_map,
    };

    let json = serde_json::to_string(&seelie_export)?;
    tracing::trace!("{json}");
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_achievements() -> Vec<Achievement> {
        vec![
            Achievement {
                id: 84002,
                status: 3,
                finish_timestamp: Some(1680000000),
            },
            Achievement {
                id: 81001,
                status: 2,
                finish_timestamp: Some(1670000000),
            },
            Achievement {
                id: 82001,
                status: 1,
                finish_timestamp: None,
            },
            Achievement {
                id: 80001,
                status: 0,
                finish_timestamp: None,
            },
            Achievement {
                id: 85001,
                status: 2,
                finish_timestamp: None,
            },
        ]
    }

    #[test]
    fn test_export_uiaf_completed_only() {
        let achievements = sample_achievements();
        let settings = AchievementExportSettings {
            format: AchievementExportFormat::Uiaf,
            completed_only: true,
        };

        let json = export_uiaf_with_timestamp(&achievements, &settings, 1700000000).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["info"]["export_app"], "Irminsul");
        assert_eq!(parsed["info"]["uiaf_version"], "v1.1");
        assert_eq!(parsed["info"]["export_timestamp"], 1700000000);

        let list = parsed["list"].as_array().unwrap();
        // Only completed (status 2 or 3) should be exported: 81001, 84002, 85001
        assert_eq!(list.len(), 3);

        // Should be sorted by ID
        assert_eq!(list[0]["id"], 81001);
        assert_eq!(list[0]["status"], 2);
        assert_eq!(list[0]["timestamp"], 1670000000);
        assert_eq!(list[0]["current"], 0);

        assert_eq!(list[1]["id"], 84002);
        assert_eq!(list[1]["status"], 3);
        assert_eq!(list[1]["timestamp"], 1680000000);
        assert_eq!(list[1]["current"], 0);

        // 85001 had no finish_timestamp, so it fell back to export_timestamp
        assert_eq!(list[2]["id"], 85001);
        assert_eq!(list[2]["status"], 2);
        assert_eq!(list[2]["timestamp"], 1700000000);
        assert_eq!(list[2]["current"], 0);

        // Also verify typed deserialization works
        let uiaf: Uiaf = serde_json::from_str(&json).unwrap();
        assert_eq!(uiaf.list.len(), 3);
        assert_eq!(uiaf.info.export_app, "Irminsul");
    }

    #[test]
    fn test_export_uiaf_all() {
        let achievements = sample_achievements();
        let settings = AchievementExportSettings {
            format: AchievementExportFormat::Uiaf,
            completed_only: false,
        };

        let json = export_uiaf_with_timestamp(&achievements, &settings, 1700000000).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let list = parsed["list"].as_array().unwrap();

        assert_eq!(list.len(), 5);
        // 80001 (status 0) and 82001 (status 1) should be included with timestamp 0
        assert_eq!(list[0]["id"], 80001);
        assert_eq!(list[0]["status"], 0);
        assert_eq!(list[0]["timestamp"], 0);

        assert_eq!(list[2]["id"], 82001);
        assert_eq!(list[2]["status"], 1);
        assert_eq!(list[2]["timestamp"], 0);
    }

    #[test]
    fn test_export_seelie_completed_only() {
        let achievements = sample_achievements();
        let settings = AchievementExportSettings {
            format: AchievementExportFormat::Seelie,
            completed_only: true,
        };

        let json = export_seelie(&achievements, &settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let achs = parsed["achievements"].as_object().unwrap();

        assert_eq!(achs.len(), 3);
        assert!(achs.contains_key("81001"));
        assert_eq!(achs["81001"]["done"], true);
        assert!(achs.contains_key("84002"));
        assert_eq!(achs["84002"]["done"], true);
        assert!(achs.contains_key("85001"));
        assert_eq!(achs["85001"]["done"], true);
        assert!(!achs.contains_key("82001"));
        assert!(!achs.contains_key("80001"));

        // Verify typed deserialization
        let seelie: SeelieExport = serde_json::from_str(&json).unwrap();
        assert_eq!(seelie.achievements.len(), 3);
    }

    #[test]
    fn test_export_seelie_all() {
        let achievements = sample_achievements();
        let settings = AchievementExportSettings {
            format: AchievementExportFormat::Seelie,
            completed_only: false,
        };

        let json = export_seelie(&achievements, &settings).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let achs = parsed["achievements"].as_object().unwrap();

        assert_eq!(achs.len(), 5);
        assert_eq!(achs["81001"]["done"], true);
        assert_eq!(achs["84002"]["done"], true);
        assert_eq!(achs["82001"]["done"], false);
        assert_eq!(achs["80001"]["done"], false);
    }

    #[test]
    fn test_export_achievements_dispatcher() {
        let achievements = sample_achievements();
        let uiaf_settings = AchievementExportSettings {
            format: AchievementExportFormat::Uiaf,
            completed_only: true,
        };
        let seelie_settings = AchievementExportSettings {
            format: AchievementExportFormat::Seelie,
            completed_only: true,
        };

        let uiaf_json = export_achievements(&achievements, &uiaf_settings).unwrap();
        let seelie_json = export_achievements(&achievements, &seelie_settings).unwrap();

        assert!(uiaf_json.contains("\"uiaf_version\":\"v1.1\""));
        assert!(seelie_json.contains("\"achievements\":{"));
    }
}
