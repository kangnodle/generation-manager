use crate::{Clock,
            Error,
            GenerationRepository,
            HealthVerifier,
            Result,
            ServiceRuntime,
            StoredGeneration};
use gm_core::{Config,
              GenerationId,
              GenerationStatus,
              RunSource,
              activation::{Activation,
                           ActivationAction,
                           ActivationEvent,
                           ActivationState}};
use std::time::Duration;

struct ActivationContext<'a> {
    target: StoredGeneration,
    previous: Option<StoredGeneration>,
    config: &'a Config,
    reason: &'a str,
}

pub fn activate_generation(
    id: GenerationId,
    reason: &str,
    config: &Config,
    repository: &impl GenerationRepository,
    runtime: &impl ServiceRuntime,
    health: &impl HealthVerifier,
    clock: &impl Clock,
) -> Result<Activation> {
    config.validate()?;
    let target = Error::port("loading target generation", repository.get(id))?;
    let current = Error::port("reading active generation", repository.current_id())?;
    let mut state = ActivationState::begin(id, current);
    let previous = state
        .previous()
        .map(|id| Error::port("loading previous generation", repository.get(id)))
        .transpose()?;
    let context = ActivationContext {
        target,
        previous,
        config,
        reason,
    };
    while let Some(action) = state.action() {
        let result = context.perform(action, repository, runtime, health, clock);
        let event = match result {
            | Ok(event) => event,
            | Err(error) if matches!(action, ActivationAction::StopCurrent | ActivationAction::RestorePointer(_)) => return Err(error),
            | Err(error) => ActivationEvent::Failed(error.to_string()),
        };
        state = state.transition(event).map_err(|error| Error::Precondition(error.to_string()))?;
    }
    state.outcome().ok_or_else(|| Error::Precondition("activation ended without an outcome".into()))
}

impl ActivationContext<'_> {
    fn perform(
        &self,
        action: ActivationAction,
        repository: &impl GenerationRepository,
        runtime: &impl ServiceRuntime,
        health: &impl HealthVerifier,
        clock: &impl Clock,
    ) -> Result<ActivationEvent> {
        use ActivationAction::*;
        let stop_timeout = Duration::from_secs(self.config.run.stop_timeout_secs);
        match action {
            | StopCurrent => {
                Error::port("stopping running service", runtime.stop_if_running(stop_timeout))?;
            },
            | SwitchTarget(id) => Error::port("switching active generation", repository.switch(id, self.reason, clock.now()))?,
            | StartTarget(id) => {
                let state = Error::port(
                    "starting generation",
                    runtime.start_detached(
                        &self.config.run,
                        &self.target.payload,
                        RunSource::Generation {
                            id,
                        },
                    ),
                )?;
                return Ok(ActivationEvent::Started(state.pid));
            },
            | VerifyTarget => Error::port("checking generation health", health.verify(&self.config.health, &self.target.payload))?,
            | MarkHealthy(id) => Error::port("marking generation healthy", repository.set_status(id, GenerationStatus::Healthy))?,
            | MarkRejected(id) => repository.set_status(id, GenerationStatus::Rejected).map_err(recovery_error)?,
            | StopRejected => {
                runtime.stop_if_running(stop_timeout).map_err(recovery_error)?;
            },
            | SwitchPrevious(id) => repository
                .switch(id, "auto-rollback after failed health check", clock.now())
                .map_err(recovery_error)?,
            | RestorePointer(id) => Error::port(
                if id.is_some() {
                    "restoring previous pointer"
                } else {
                    "clearing rejected pointer"
                },
                repository.restore(id),
            )?,
            | StartPrevious(id) => {
                runtime
                    .start_detached(
                        &self.config.run,
                        &self.previous()?.payload,
                        RunSource::Generation {
                            id,
                        },
                    )
                    .map_err(recovery_error)?;
            },
            | VerifyPrevious => health.verify(&self.config.health, &self.previous()?.payload).map_err(recovery_error)?,
        }
        Ok(ActivationEvent::Succeeded)
    }

    fn previous(&self) -> Result<&StoredGeneration> {
        self.previous
            .as_ref()
            .ok_or_else(|| Error::Precondition("previous generation is unavailable".into()))
    }
}

fn recovery_error(source: crate::BoxError) -> Error { Error::Precondition(source.to_string()) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoxError,
                Clock,
                GenerationRepository,
                HealthVerifier,
                PortResult,
                ServiceRuntime,
                StagedGeneration,
                StoredGeneration};
    use gm_core::{Generation,
                  GenerationStatus,
                  Preset,
                  RunState};
    use std::{cell::{Cell,
                     RefCell},
              collections::BTreeMap,
              path::{Path,
                     PathBuf},
              time::{Duration,
                     SystemTime}};

    struct Repository {
        entries: BTreeMap<GenerationId, StoredGeneration>,
        current: Cell<Option<GenerationId>>,
        statuses: RefCell<Vec<(GenerationId, GenerationStatus)>>,
        switches: RefCell<Vec<GenerationId>>,
        fail_switch: bool,
        fail_status: bool,
    }

    impl GenerationRepository for Repository {
        fn get(&self, id: GenerationId) -> PortResult<StoredGeneration> { self.entries.get(&id).cloned().ok_or_else(|| failure("missing generation")) }

        fn current_id(&self) -> PortResult<Option<GenerationId>> { Ok(self.current.get()) }

        fn stage(&self, _commit: Option<&str>) -> PortResult<StagedGeneration> { Err(failure("unused")) }

        fn discard(&self, _staged: StagedGeneration) -> PortResult<()> { Err(failure("unused")) }

        fn commit(&self, _staged: StagedGeneration, _generation: Generation) -> PortResult<StoredGeneration> { Err(failure("unused")) }

        fn set_status(&self, id: GenerationId, status: GenerationStatus) -> PortResult<()> {
            if self.fail_status {
                return Err(failure("status write failed"));
            }
            self.statuses.borrow_mut().push((id, status));
            Ok(())
        }

        fn switch(&self, id: GenerationId, _reason: &str, _at: SystemTime) -> PortResult<()> {
            self.current.set(Some(id));
            self.switches.borrow_mut().push(id);
            if self.fail_switch {
                return Err(failure("history write failed"));
            }
            Ok(())
        }

        fn restore(&self, id: Option<GenerationId>) -> PortResult<()> {
            self.current.set(id);
            Ok(())
        }
    }

    struct Runtime {
        fail_for: Option<GenerationId>,
        starts: RefCell<Vec<GenerationId>>,
        stops: Cell<usize>,
    }

    impl ServiceRuntime for Runtime {
        fn stop_if_running(&self, _timeout: Duration) -> PortResult<Option<RunState>> {
            self.stops.set(self.stops.get() + 1);
            Ok(None)
        }

        fn start_detached(&self, _run: &gm_core::RunStage, _cwd: &Path, source: RunSource) -> PortResult<RunState> {
            let RunSource::Generation {
                id,
            } = source
            else {
                return Err(failure("unexpected worktree"));
            };
            self.starts.borrow_mut().push(id);
            if self.fail_for == Some(id) {
                return Err(failure("start failed"));
            }
            Ok(RunState {
                source: RunSource::Generation {
                    id,
                },
                pid: id.0 as i32,
                process_start: None,
                started_at: fixed_time(),
                detached: true,
            })
        }
    }

    struct Health {
        fail_for: Option<PathBuf>,
    }

    impl HealthVerifier for Health {
        fn verify(&self, _check: &gm_core::HealthCheck, cwd: &Path) -> PortResult<()> {
            if self.fail_for.as_deref() == Some(cwd) {
                Err(failure("unhealthy"))
            } else {
                Ok(())
            }
        }
    }

    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> SystemTime { fixed_time() }
    }

    fn fixed_time() -> SystemTime { SystemTime::UNIX_EPOCH }

    fn failure(message: &str) -> BoxError { std::io::Error::other(message).into() }

    fn repository(current: Option<GenerationId>) -> Repository {
        let entries = [1, 2]
            .into_iter()
            .map(|value| {
                let id = GenerationId(value);
                let payload = PathBuf::from(format!("/generation/{value}"));
                (
                    id,
                    StoredGeneration {
                        meta: Generation {
                            id,
                            commit: None,
                            worktree: None,
                            dirty: false,
                            built_at: fixed_time(),
                            status: GenerationStatus::Built,
                            artifacts: Vec::new(),
                            note: None,
                        },
                        payload,
                    },
                )
            })
            .collect();
        Repository {
            entries,
            current: Cell::new(current),
            statuses: RefCell::new(Vec::new()),
            switches: RefCell::new(Vec::new()),
            fail_switch: false,
            fail_status: false,
        }
    }

    #[test]
    fn healthy_activation_updates_status_without_rollback() {
        let repository = repository(Some(GenerationId(1)));
        let runtime = Runtime {
            fail_for: None,
            starts: RefCell::new(Vec::new()),
            stops: Cell::new(0),
        };
        let result = match activate_generation(
            GenerationId(2),
            "test",
            &Preset::Generic.template("app"),
            &repository,
            &runtime,
            &Health {
                fail_for: None,
            },
            &FixedClock,
        ) {
            | Ok(outcome) => outcome,
            | Err(error) => panic!("activation did not return an outcome: {error}"),
        };

        assert!(matches!(
            result,
            Activation::Healthy {
                id: GenerationId(2),
                ..
            }
        ));
        assert_eq!(repository.current.get(), Some(GenerationId(2)));
        assert_eq!(*repository.statuses.borrow(), vec![(GenerationId(2), GenerationStatus::Healthy)]);
    }

    #[test]
    fn failed_health_check_rejects_and_restores_the_previous_generation() {
        let repository = repository(Some(GenerationId(1)));
        let runtime = Runtime {
            fail_for: None,
            starts: RefCell::new(Vec::new()),
            stops: Cell::new(0),
        };
        let result = match activate_generation(
            GenerationId(2),
            "test",
            &Preset::Generic.template("app"),
            &repository,
            &runtime,
            &Health {
                fail_for: Some(PathBuf::from("/generation/2")),
            },
            &FixedClock,
        ) {
            | Ok(outcome) => outcome,
            | Err(error) => panic!("activation did not return an outcome: {error}"),
        };

        assert!(matches!(
            result,
            Activation::RolledBack {
                failed: GenerationId(2),
                restored: Some(GenerationId(1)),
                restored_healthy: true,
                ..
            }
        ));
        assert_eq!(repository.current.get(), Some(GenerationId(1)));
        assert_eq!(*repository.switches.borrow(), vec![GenerationId(2), GenerationId(1)]);
        assert_eq!(*repository.statuses.borrow(), vec![(GenerationId(2), GenerationStatus::Rejected)]);
    }

    #[test]
    fn bookkeeping_failures_do_not_prevent_restoring_the_service() {
        for (fail_switch, fail_status, fail_health) in [(true, false, false), (false, true, true), (false, true, false)] {
            let mut repository = repository(Some(GenerationId(1)));
            repository.fail_switch = fail_switch;
            repository.fail_status = fail_status;
            let runtime = Runtime {
                fail_for: None,
                starts: RefCell::new(Vec::new()),
                stops: Cell::new(0),
            };
            let result = match activate_generation(
                GenerationId(2),
                "test",
                &Preset::Generic.template("app"),
                &repository,
                &runtime,
                &Health {
                    fail_for: fail_health.then(|| PathBuf::from("/generation/2")),
                },
                &FixedClock,
            ) {
                | Ok(outcome) => outcome,
                | Err(error) => panic!("activation did not return an outcome: {error}"),
            };
            assert!(matches!(
                result,
                Activation::RolledBack {
                    restored: Some(GenerationId(1)),
                    restored_healthy: true,
                    ..
                }
            ));
            assert_eq!(repository.current.get(), Some(GenerationId(1)));
            assert_eq!(runtime.starts.borrow().last(), Some(&GenerationId(1)));
        }
    }

    #[test]
    fn failed_first_activation_clears_the_rejected_pointer() {
        let repository = repository(None);
        let runtime = Runtime {
            fail_for: Some(GenerationId(2)),
            starts: RefCell::new(Vec::new()),
            stops: Cell::new(0),
        };
        match activate_generation(
            GenerationId(2),
            "test",
            &Preset::Generic.template("app"),
            &repository,
            &runtime,
            &Health {
                fail_for: None,
            },
            &FixedClock,
        ) {
            | Ok(outcome) => outcome,
            | Err(error) => panic!("activation did not return an outcome: {error}"),
        };
        assert_eq!(repository.current.get(), None);
    }
    #[test]
    fn service_start_and_restart_do_not_change_the_activation_pointer() {
        let repository = repository(Some(GenerationId(1)));
        let runtime = Runtime {
            fail_for: None,
            starts: RefCell::new(Vec::new()),
            stops: Cell::new(0),
        };
        let config = Preset::Generic.template("app");
        let started = match crate::start_service(&config, &repository, &runtime, false) {
            | Ok(started) => started,
            | Err(error) => panic!("failed to start service: {error}"),
        };
        assert_eq!(started.state.pid, 1);
        assert_eq!(runtime.stops.get(), 0);
        match crate::start_service(&config, &repository, &runtime, true) {
            | Ok(started) => started,
            | Err(error) => panic!("failed to restart service: {error}"),
        };
        assert_eq!(runtime.stops.get(), 1);
        assert_eq!(repository.current.get(), Some(GenerationId(1)));
        assert!(repository.switches.borrow().is_empty());
        assert!(repository.statuses.borrow().is_empty());
    }

    #[test]
    fn service_restart_checks_the_payload_before_stopping_the_existing_run() {
        let repository = repository(Some(GenerationId(99)));
        let runtime = Runtime {
            fail_for: None,
            starts: RefCell::new(Vec::new()),
            stops: Cell::new(0),
        };
        assert!(crate::start_service(&Preset::Generic.template("app"), &repository, &runtime, true).is_err());
        assert_eq!(runtime.stops.get(), 0);
        assert!(runtime.starts.borrow().is_empty());
    }
}
