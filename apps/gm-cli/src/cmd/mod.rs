mod composition;
mod dev;
mod init;
mod lifecycle;
mod project;
mod service;

use crate::{Command,
            GenerationCommand,
            ProjectCommand,
            ServiceCommand,
            WorktreeCommand};
use anyhow::{Result,
             bail};
use gm_core::config::Config;
use gm_store::{Layout,
               Store};
use std::{path::PathBuf,
          process::ExitCode};

pub struct Project {
    pub config: Config,
    pub store: Store,
    pub root: PathBuf,
    pub worktree: Option<String>,
}

impl Project {
    pub fn open() -> Result<Project> {
        let cwd = std::env::current_dir()?;
        let found = gm_store::discover_project(&cwd)?;
        let store = Store::open(Layout::new(&found.root))?;
        Ok(Project {
            config: found.config,
            store,
            root: found.root,
            worktree: found.worktree,
        })
    }

    pub fn select_worktree(&self, from: Option<&str>) -> Option<String> { gm_core::select_worktree(from, self.worktree.as_deref()) }

    pub fn worktree_path(&self, name: &str) -> Result<PathBuf> {
        let path = self.store.layout().worktree(name)?;
        if !path.is_dir() {
            bail!("worktree `{name}` does not exist (see `gm worktree list`)");
        }
        Ok(path)
    }
}

pub fn dispatch(command: Command) -> Result<ExitCode> {
    match command {
        | Command::Project(ProjectCommand::Init {
            preset,
            name,
            force,
        }) => init::run(preset, name, force),
        | Command::Project(ProjectCommand::Status) => project::status(),

        | Command::Worktree(WorktreeCommand::Create {
            name,
            base,
        }) => dev::new(&name, base.as_deref()),
        | Command::Worktree(WorktreeCommand::List) => dev::list(),
        | Command::Worktree(WorktreeCommand::Run {
            worktree,
            no_build,
            detach,
        }) => dev::run(worktree.as_deref(), no_build, detach),
        | Command::Worktree(WorktreeCommand::Remove {
            name,
            force,
        }) => dev::remove(&name, force),

        | Command::Generation(GenerationCommand::Build {
            worktree,
            note,
            activate,
        }) => lifecycle::build(worktree.as_deref(), note, activate),
        | Command::Generation(GenerationCommand::List) => lifecycle::generations(),
        | Command::Generation(GenerationCommand::Activate {
            generation,
        }) => lifecycle::activate_generation(generation),
        | Command::Generation(GenerationCommand::Rollback {
            generation,
        }) => lifecycle::rollback(generation),
        | Command::Generation(GenerationCommand::History) => lifecycle::history(),
        | Command::Generation(GenerationCommand::Prune {
            keep,
        }) => lifecycle::prune(keep),

        | Command::Service(ServiceCommand::Status) => service::status(),
        | Command::Service(ServiceCommand::Start) => service::start(),
        | Command::Service(ServiceCommand::Stop) => service::stop(),
        | Command::Service(ServiceCommand::Restart) => service::restart(),
        | Command::Service(ServiceCommand::Logs {
            lines,
        }) => service::logs(lines),
    }
}
