use crate::generation::GenerationId;
use std::fmt;

/// Where the running process came from.
///
/// Recording this is what keeps `gm project status` honest once a worktree can
/// be run directly: "something is running" is not useful if you cannot tell a
/// verified generation from an unverified development checkout.
#[derive(Debug, Clone, PartialEq, Eq)]

pub enum RunSource {
    /// The activated generation — built, tested and health-checked.
    Generation { id: GenerationId },
    /// A development worktree, run in place with no verification.
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

/// The single service slot. One project runs at most one process, so this file
/// replaces a bare pid file: the pid alone cannot answer "running what?".
#[derive(Debug, Clone)]
pub struct RunState {
    pub source: RunSource,
    pub pid: i32,
    /// Kernel start identity; absent in state written by older versions.
    pub process_start: Option<String>,
    pub started_at: std::time::SystemTime,
    /// False while a foreground `gm worktree run` owns the terminal.
    pub detached: bool,
}
