use gm_core::{Generation,
              GenerationId,
              GenerationStatus,
              RunSource,
              RunState,
              SwitchEvent};
use serde::{Deserialize,
            Serialize};
use std::{path::PathBuf,
          time::SystemTime};

#[derive(Serialize, Deserialize)]
#[serde(remote = "GenerationId")]
#[serde(transparent)]
pub struct GenerationIdFormat(pub u64);

#[derive(Serialize, Deserialize)]
#[serde(remote = "GenerationStatus")]
#[serde(rename_all = "kebab-case")]
pub enum GenerationStatusFormat {
    Built,
    Healthy,
    Rejected,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Generation")]
pub struct GenerationFormat {
    #[serde(with = "GenerationIdFormat")]
    pub id: GenerationId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(default)]
    pub dirty: bool,
    #[serde(with = "timestamp")]
    pub built_at: SystemTime,
    #[serde(with = "GenerationStatusFormat")]
    pub status: GenerationStatus,
    #[serde(default)]
    pub artifacts: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct GenerationRecord(#[serde(with = "GenerationFormat")] pub Generation);

#[derive(Serialize, Deserialize)]
#[serde(remote = "SwitchEvent")]
pub struct SwitchEventFormat {
    #[serde(with = "timestamp")]
    pub at: SystemTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "optional_id")]
    pub from: Option<GenerationId>,
    #[serde(with = "GenerationIdFormat")]
    pub to: GenerationId,
    pub reason: String,
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct SwitchEventRecord(#[serde(with = "SwitchEventFormat")] pub SwitchEvent);

#[derive(Serialize, Deserialize)]
#[serde(remote = "RunSource")]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RunSourceFormat {
    Generation {
        #[serde(with = "GenerationIdFormat")]
        id: GenerationId,
    },
    Worktree {
        name: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "RunState")]
pub struct RunStateFormat {
    #[serde(with = "RunSourceFormat")]
    pub source: RunSource,
    pub pid: i32,
    #[serde(default)]
    pub process_start: Option<String>,
    #[serde(with = "timestamp")]
    pub started_at: SystemTime,
    #[serde(default)]
    pub detached: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct RunStateRecord(#[serde(with = "RunStateFormat")] pub RunState);

mod optional_id {
    use gm_core::GenerationId;
    use serde::{Deserialize,
                Deserializer,
                Serialize,
                Serializer};

    pub fn serialize<S: Serializer>(id: &Option<GenerationId>, serializer: S) -> Result<S::Ok, S::Error> { id.map(|id| id.0).serialize(serializer) }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<GenerationId>, D::Error> {
        Option::<u64>::deserialize(deserializer).map(|id| id.map(GenerationId))
    }
}

mod timestamp {
    use chrono::{DateTime,
                 Utc};
    use serde::{Deserialize,
                Deserializer,
                Serialize,
                Serializer};
    use std::time::SystemTime;

    pub fn serialize<S: Serializer>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error> { DateTime::<Utc>::from(*time).serialize(serializer) }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<SystemTime, D::Error> {
        DateTime::<Utc>::deserialize(deserializer).map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_generation_metadata_keeps_its_schema() {
        let json = r#"{"id":12,"built_at":"2026-08-28T00:00:00.123Z","status":"built"}"#;
        let record: GenerationRecord = match serde_json::from_str(json) {
            | Ok(record) => record,
            | Err(error) => panic!("failed to decode legacy generation metadata: {error}"),
        };
        assert_eq!(record.0.id, GenerationId(12));
        assert_eq!(record.0.status, GenerationStatus::Built);
        assert!(!record.0.dirty);
        assert!(record.0.artifacts.is_empty());
        let encoded = match serde_json::to_value(record) {
            | Ok(value) => value,
            | Err(error) => panic!("failed to encode record: {error}"),
        };
        assert_eq!(encoded["id"], 12);
        assert_eq!(encoded["built_at"], "2026-08-28T00:00:00.123Z");
        assert_eq!(encoded["status"], "built");
        assert!(encoded.get("commit").is_none());
    }

    #[test]
    fn legacy_switch_history_supports_an_omitted_previous_generation() {
        for json in [
            r#"{"at":"2026-08-28T00:00:00Z","to":1,"reason":"first"}"#,
            r#"{"at":"2026-08-28T00:00:01Z","from":1,"to":2,"reason":"test"}"#,
        ] {
            let record: SwitchEventRecord = match serde_json::from_str(json) {
                | Ok(record) => record,
                | Err(error) => panic!("failed to decode legacy switch event: {error}"),
            };
            let encoded = match serde_json::to_value(record) {
                | Ok(value) => value,
                | Err(error) => panic!("failed to encode record: {error}"),
            };
            let expected: serde_json::Value = match serde_json::from_str(json) {
                | Ok(value) => value,
                | Err(error) => panic!("invalid switch event JSON fixture: {error}"),
            };
            assert_eq!(encoded, expected);
        }
    }

    #[test]
    fn legacy_run_state_defaults_the_process_identity_and_detach_flag() {
        let json = r#"{"source":{"kind":"generation","id":3},"pid":41,"started_at":"2026-08-28T00:00:00Z"}"#;
        let record: RunStateRecord = match serde_json::from_str(json) {
            | Ok(record) => record,
            | Err(error) => panic!("failed to decode legacy run state: {error}"),
        };
        assert_eq!(
            record.0.source,
            RunSource::Generation {
                id: GenerationId(3)
            }
        );
        assert_eq!(record.0.process_start, None);
        assert!(!record.0.detached);
        let encoded = match serde_json::to_value(record) {
            | Ok(value) => value,
            | Err(error) => panic!("failed to encode record: {error}"),
        };
        assert_eq!(encoded["source"], serde_json::json!({"kind": "generation", "id": 3}));
        assert_eq!(encoded["started_at"], "2026-08-28T00:00:00Z");
    }
}
