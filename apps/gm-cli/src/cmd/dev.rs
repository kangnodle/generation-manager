use crate::{cmd::Project,
            ui};
use anyhow::{Result,
             bail};
use gm_application::{RunWorktree,
                     WorktreeRun,
                     create_worktree,
                     remove_worktree,
                     run_worktree};
use gm_runner::{Supervisor,
                exec::ShellExecutor,
                git};
use std::process::ExitCode;

pub fn new(name: &str, base: Option<&str>) -> Result<ExitCode> {
    let project = Project::open()?;
    let lock = project.store.lock()?;

    let repository = project.store.repository(&lock)?;
    let path = create_worktree(name, base, &repository, &git::GitSourceControl)?;

    println!("{} worktree `{name}` at {}", ui::OK, path.display());
    println!("  branch       {name}");
    println!();
    println!("  cd {}", path.display());
    println!("  # …develop, commit…");
    println!("  gm generation build {name} --activate");
    Ok(ExitCode::SUCCESS)
}

pub fn run(from: Option<&str>, no_build: bool, detach: bool) -> Result<ExitCode> {
    let project = Project::open()?;

    let Some(name) = project.select_worktree(from) else {
        bail!(
            "not inside a worktree — pass `<worktree>`, or use `gm service start` to run the \
             active generation"
        );
    };
    let path = project.worktree_path(&name)?;

    let lock = project.store.lock()?;
    let supervisor = project.supervisor();
    report_displacement(&supervisor);

    println!("running worktree `{name}` from {}", path.display());
    if detach {
        println!("  {} unverified: no health check, no rollback", ui::ARROW);
    }
    println!();

    let started = run_worktree(
        RunWorktree {
            name: &name,
            path: &path,
            build: !no_build,
            detach,
        },
        &project.config,
        &ShellExecutor,
        &supervisor,
    )?;
    drop(lock);
    match started {
        | WorktreeRun::Detached(state) => {
            println!();
            println!("{} worktree `{name}` running in the background (pid {})", ui::OK, state.pid);
            println!("  logs with `gm service logs`, stop with `gm service stop`");
            Ok(ExitCode::SUCCESS)
        },
        | WorktreeRun::Foreground(running) => {
            let code = running.wait()?;
            println!();
            if code != 0 {
                eprintln!("{} worktree `{name}` exited with code {code}", ui::ERR);
            }
            if let Ok(Some(id)) = project.store.current_id() {
                println!("  service slot is free — `gm service start` runs generation {id} again");
            }
            Ok(ExitCode::from(code as u8))
        },
    }
}

fn report_displacement(supervisor: &Supervisor) {
    if let Some(state) = supervisor.running() {
        println!("  {} stopping {} to take the service slot", ui::ARROW, state.source);
    }
}

pub fn list() -> Result<ExitCode> {
    let project = Project::open()?;

    let mut rows = Vec::new();
    for (name, path) in project.store.list_worktrees()? {
        let branch = git::current_branch(&path).unwrap_or_else(|| "?".into());
        let commit: String = git::head_commit(&path).unwrap_or_default().chars().take(8).collect();
        let dirty = if git::is_dirty(&path) { "dirty" } else { "clean" };
        let here = project.worktree.as_deref() == Some(name.as_str());
        rows.push((name, branch, commit, dirty, here));
    }

    if rows.is_empty() {
        println!("no worktrees (create one with `gm worktree create <name>`)");
        return Ok(ExitCode::SUCCESS);
    }

    rows.sort();
    println!("{:<3} {:<20} {:<20} {:<10} STATE", "", "NAME", "BRANCH", "COMMIT");
    for (name, branch, commit, dirty, here) in rows {
        let marker = if here { "*" } else { " " };
        println!("{marker:<3} {name:<20} {branch:<20} {commit:<10} {dirty}");
    }
    Ok(ExitCode::SUCCESS)
}

pub fn remove(name: &str, force: bool) -> Result<ExitCode> {
    let project = Project::open()?;
    let lock = project.store.lock()?;

    let repository = project.store.repository(&lock)?;
    remove_worktree(name, force, &repository, &git::GitSourceControl)?;

    println!("{} removed worktree `{name}`", ui::OK);
    println!("  the branch `{name}` was kept; delete it with `git branch -D {name}`");
    Ok(ExitCode::SUCCESS)
}
