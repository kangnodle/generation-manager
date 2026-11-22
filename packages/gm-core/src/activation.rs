use crate::{GenerationId,
            activation_restore_target};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activation {
    Healthy {
        id: GenerationId,
        pid: i32,
    },
    RolledBack {
        failed: GenerationId,
        restored: Option<GenerationId>,
        restored_healthy: bool,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationAction {
    StopCurrent,
    SwitchTarget(GenerationId),
    StartTarget(GenerationId),
    VerifyTarget,
    MarkHealthy(GenerationId),
    MarkRejected(GenerationId),
    StopRejected,
    SwitchPrevious(GenerationId),
    RestorePointer(Option<GenerationId>),
    StartPrevious(GenerationId),
    VerifyPrevious,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationEvent {
    Succeeded,
    Started(i32),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    StopCurrent,
    SwitchTarget,
    StartTarget,
    VerifyTarget(i32),
    MarkHealthy(i32),
    Recover(Recovery, String),
    Complete(Activation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recovery {
    Reject,
    Stop,
    Switch(GenerationId),
    Restore(Option<GenerationId>),
    Start(GenerationId),
    Verify,
}

#[derive(Debug, Clone)]
pub struct ActivationState {
    target: GenerationId,
    previous: Option<GenerationId>,
    phase: Phase,
}

#[derive(Debug)]
pub struct InvalidActivationEvent;

impl fmt::Display for InvalidActivationEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter.write_str("unexpected event for the activation phase") }
}

impl std::error::Error for InvalidActivationEvent {}

impl ActivationState {
    pub fn begin(target: GenerationId, current: Option<GenerationId>) -> Self {
        Self {
            target,
            previous: activation_restore_target(current, target),
            phase: Phase::StopCurrent,
        }
    }

    pub fn previous(&self) -> Option<GenerationId> { self.previous }

    pub fn outcome(&self) -> Option<Activation> {
        match &self.phase {
            | Phase::Complete(outcome) => Some(outcome.clone()),
            | _ => None,
        }
    }

    pub fn action(&self) -> Option<ActivationAction> {
        use ActivationAction as Action;
        Some(match self.phase {
            | Phase::StopCurrent => Action::StopCurrent,
            | Phase::SwitchTarget => Action::SwitchTarget(self.target),
            | Phase::StartTarget => Action::StartTarget(self.target),
            | Phase::VerifyTarget(_) => Action::VerifyTarget,
            | Phase::MarkHealthy(_) => Action::MarkHealthy(self.target),
            | Phase::Recover(step, _) => match step {
                | Recovery::Reject => Action::MarkRejected(self.target),
                | Recovery::Stop => Action::StopRejected,
                | Recovery::Switch(id) => Action::SwitchPrevious(id),
                | Recovery::Restore(id) => Action::RestorePointer(id),
                | Recovery::Start(id) => Action::StartPrevious(id),
                | Recovery::Verify => Action::VerifyPrevious,
            },
            | Phase::Complete(_) => return None,
        })
    }

    pub fn transition(mut self, event: ActivationEvent) -> Result<Self, InvalidActivationEvent> {
        use ActivationEvent::{Failed,
                              Started,
                              Succeeded};
        self.phase = match (self.phase, event) {
            | (Phase::StopCurrent, Succeeded) => Phase::SwitchTarget,
            | (Phase::SwitchTarget, Succeeded) => Phase::StartTarget,
            | (Phase::StartTarget, Started(pid)) => Phase::VerifyTarget(pid),
            | (Phase::VerifyTarget(pid), Succeeded) => Phase::MarkHealthy(pid),
            | (Phase::MarkHealthy(pid), Succeeded) => Phase::Complete(Activation::Healthy {
                id: self.target,
                pid,
            }),
            | (Phase::SwitchTarget | Phase::StartTarget | Phase::VerifyTarget(_) | Phase::MarkHealthy(_), Failed(reason)) =>
                Phase::Recover(Recovery::Reject, reason),
            | (Phase::Recover(step, reason), Succeeded) => recover(self.target, self.previous, step, reason, None),
            | (Phase::Recover(step, reason), Failed(error)) if !matches!(step, Recovery::Restore(_)) =>
                recover(self.target, self.previous, step, reason, Some(error)),
            | _ => return Err(InvalidActivationEvent),
        };
        Ok(self)
    }
}

fn recover(target: GenerationId, previous: Option<GenerationId>, step: Recovery, mut reason: String, error: Option<String>) -> Phase {
    let succeeded = error.is_none();
    append_recovery_error(&mut reason, step, error);
    let next = match step {
        | Recovery::Reject => Recovery::Stop,
        | Recovery::Stop => previous.map_or(Recovery::Restore(None), Recovery::Switch),
        | Recovery::Switch(id) if !succeeded => Recovery::Restore(Some(id)),
        | Recovery::Switch(id) | Recovery::Restore(Some(id)) => Recovery::Start(id),
        | Recovery::Start(_) if succeeded => Recovery::Verify,
        | Recovery::Restore(None) | Recovery::Start(_) | Recovery::Verify => {
            return Phase::Complete(Activation::RolledBack {
                failed: target,
                restored: previous,
                restored_healthy: step == Recovery::Verify && succeeded,
                reason,
            });
        },
    };
    Phase::Recover(next, reason)
}

fn append_recovery_error(reason: &mut String, step: Recovery, error: Option<String>) {
    if let Some(error) = error {
        let operation = match step {
            | Recovery::Reject => "marking rejected failed",
            | Recovery::Stop => "stopping rejected generation failed",
            | Recovery::Switch(_) => "recording rollback failed",
            | Recovery::Start(_) | Recovery::Verify => "restarting previous generation failed",
            | Recovery::Restore(_) => "restoring pointer failed",
        };
        reason.push_str(&format!("; {operation}: {error}"));
    }
}
