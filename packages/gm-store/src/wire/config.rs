use gm_core::config::{Artifacts,
                      BuildStage,
                      Config,
                      HealthCheck,
                      Project,
                      RunStage};
use serde::{Deserialize,
            Serialize};
use std::{collections::BTreeMap,
          path::PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(remote = "Config")]
pub struct ConfigFormat {
    #[serde(with = "ProjectFormat")]
    pub project: Project,
    #[serde(default)]
    #[serde(with = "BuildStageFormat")]
    pub build: BuildStage,
    #[serde(default)]
    #[serde(with = "BuildStageFormat")]
    pub test: BuildStage,
    #[serde(default)]
    #[serde(with = "ArtifactsFormat")]
    pub artifacts: Artifacts,
    #[serde(with = "RunStageFormat")]
    pub run: RunStage,
    #[serde(default)]
    #[serde(with = "HealthCheckFormat")]
    pub health: HealthCheck,
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ConfigRecord(#[serde(with = "ConfigFormat")] pub Config);

#[derive(Serialize, Deserialize)]
#[serde(remote = "Project")]
pub struct ProjectFormat {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "BuildStage")]
pub struct BuildStageFormat {
    #[serde(default)]
    pub cmd: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Artifacts")]
pub struct ArtifactsFormat {
    #[serde(default)]
    pub include: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "RunStage")]
pub struct RunStageFormat {
    pub cmd: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "default_stop_timeout")]
    pub stop_timeout_secs: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "HealthCheck")]
pub struct HealthCheckFormat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default = "default_health_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_health_interval")]
    pub interval_secs: u64,
}

fn default_stop_timeout() -> u64 { RunStage::DEFAULT_STOP_TIMEOUT_SECS }

fn default_health_timeout() -> u64 { HealthCheck::DEFAULT_TIMEOUT_SECS }

fn default_health_interval() -> u64 { HealthCheck::DEFAULT_INTERVAL_SECS }

#[cfg(test)]
mod tests {
    use crate::parse_config;

    #[test]
    fn manifest_keeps_the_defaults_for_a_partially_configured_health_check() {
        let config = parse_config(
            r#"
[project]
name = "demo"

[run]
cmd = "./start"

[health]
cmd = "./check"
"#,
        )
        .unwrap();
        assert_eq!(config.run.stop_timeout_secs, 10);
        assert_eq!(config.health.timeout_secs, 30);
        assert_eq!(config.health.interval_secs, 1);
        assert!(config.build.is_empty());
        assert!(config.artifacts.include.is_empty());
    }

    #[test]
    fn manifest_keeps_the_defaults_for_an_absent_health_check() {
        let config = parse_config(
            r#"
[project]
name = "demo"

[run]
cmd = "./start"
"#,
        )
        .unwrap();
        assert!(!config.health.is_configured());
        assert_eq!(config.health.timeout_secs, 0);
        assert_eq!(config.health.interval_secs, 0);
    }
}
