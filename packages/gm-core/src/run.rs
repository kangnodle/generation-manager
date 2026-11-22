use crate::generation::GenerationId;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum RunSource {
    Generation { id: GenerationId },
    Worktree { name: String },
}

impl fmt::Display for RunSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            | RunSource::Generation {
                id,
            } => write!(f, "generation {id}"),
            | RunSource::Worktree {
                name,
            } => write!(f, "worktree `{name}` (dev, unverified)"),
        }
    }
}

impl RunSource {
    pub fn is_dev(&self) -> bool { matches!(self, RunSource::Worktree { .. }) }
}

#[derive(Debug, Clone)]
pub struct RunState {
    pub source: RunSource,
    pub pid: i32,
    pub process_start: Option<String>,
    pub started_at: std::time::SystemTime,
    pub detached: bool,
}
