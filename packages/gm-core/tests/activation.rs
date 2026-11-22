use gm_core::{GenerationId,
              activation::{Activation,
                           ActivationAction as Action,
                           ActivationEvent as Event,
                           ActivationState}};

fn advance(state: ActivationState, event: Event) -> ActivationState {
    match state.transition(event) {
        | Ok(next_state) => next_state,
        | Err(error) => panic!("activation transition failed: {error}"),
    }
}

#[test]
fn activation_requires_health_and_persisted_status_before_success() {
    let target = GenerationId(2);
    let state = ActivationState::begin(target, Some(GenerationId(1)));
    assert_eq!(state.action(), Some(Action::StopCurrent));
    assert!(state.clone().transition(Event::Started(42)).is_err());
    let state = advance(state, Event::Succeeded);
    assert_eq!(state.action(), Some(Action::SwitchTarget(target)));
    let state = advance(state, Event::Succeeded);
    assert_eq!(state.action(), Some(Action::StartTarget(target)));
    let state = advance(state, Event::Started(42));
    assert_eq!(state.action(), Some(Action::VerifyTarget));
    let state = advance(state, Event::Succeeded);
    assert_eq!(state.action(), Some(Action::MarkHealthy(target)));
    let state = advance(state, Event::Succeeded);
    assert_eq!(
        state.outcome(),
        Some(Activation::Healthy {
            id: target,
            pid: 42
        })
    );
}

#[test]
fn rollback_restores_the_pointer_after_a_failed_audit_write() {
    let target = GenerationId(2);
    let previous = GenerationId(1);
    let state = advance(ActivationState::begin(target, Some(previous)), Event::Succeeded);
    let state = advance(state, Event::Failed("switch failed".into()));
    assert_eq!(state.action(), Some(Action::MarkRejected(target)));
    let state = advance(state, Event::Failed("status write failed".into()));
    assert_eq!(state.action(), Some(Action::StopRejected));
    let state = advance(state, Event::Succeeded);
    assert_eq!(state.action(), Some(Action::SwitchPrevious(previous)));
    let state = advance(state, Event::Failed("audit failed".into()));
    assert_eq!(state.action(), Some(Action::RestorePointer(Some(previous))));
    let state = advance(state, Event::Succeeded);
    assert_eq!(state.action(), Some(Action::StartPrevious(previous)));
    let state = advance(state, Event::Succeeded);
    assert_eq!(state.action(), Some(Action::VerifyPrevious));
    let state = advance(state, Event::Failed("unhealthy".into()));
    assert_eq!(state.outcome(), Some(Activation::RolledBack {
        failed: target,
        restored: Some(previous),
        restored_healthy: false,
        reason: "switch failed; marking rejected failed: status write failed; recording rollback failed: audit failed; restarting previous generation failed: unhealthy".into(),
    }));
}

#[test]
fn failure_reactivating_the_same_generation_clears_the_pointer() {
    let target = GenerationId(2);
    let state = advance(ActivationState::begin(target, Some(target)), Event::Succeeded);
    let state = advance(state, Event::Succeeded);
    let state = advance(state, Event::Failed("start failed".into()));
    let state = advance(state, Event::Succeeded);
    let state = advance(state, Event::Succeeded);
    assert_eq!(state.action(), Some(Action::RestorePointer(None)));
    assert_eq!(
        advance(state, Event::Succeeded).outcome(),
        Some(Activation::RolledBack {
            failed: target,
            restored: None,
            restored_healthy: false,
            reason: "start failed".into(),
        })
    );
}
