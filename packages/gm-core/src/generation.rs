use std::{fmt,
          path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]

pub struct GenerationId(pub u64);

impl fmt::Display for GenerationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}

impl GenerationId {
    pub fn dir_prefix(&self) -> String { format!("{:04}", self.0) }

    pub fn next(&self) -> GenerationId { GenerationId(self.0 + 1) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum GenerationStatus {
    Built,
    Healthy,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct Generation {
    pub id: GenerationId,
    pub commit: Option<String>,
    pub worktree: Option<String>,
    pub dirty: bool,
    pub built_at: std::time::SystemTime,
    pub status: GenerationStatus,
    pub artifacts: Vec<PathBuf>,
    pub note: Option<String>,
}

impl Generation {
    pub fn describe_source(&self) -> String {
        let commit = self.commit.as_deref().unwrap_or("unknown");
        let short: String = commit.chars().take(8).collect();
        match (&self.worktree, self.dirty) {
            | (Some(w), true) => format!("{short}+dirty ({w})"),
            | (Some(w), false) => format!("{short} ({w})"),
            | (None, true) => format!("{short}+dirty"),
            | (None, false) => short,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SwitchEvent {
    pub at: std::time::SystemTime,
    pub from: Option<GenerationId>,
    pub to: GenerationId,
    pub reason: String,
}
