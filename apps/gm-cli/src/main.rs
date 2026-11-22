mod cmd;
mod ui;

use clap::{Parser,
           Subcommand};
use std::{path::PathBuf,
          process::ExitCode};

#[derive(Parser)]
#[command(
    name = "gm",
    version,
    about = "Worktree-to-generation development lifecycle manager",
    long_about = "Develop in an isolated git worktree, freeze a build into an immutable \
                  generation, activate it with a health check, and roll back when it \
                  misbehaves."
)]
struct Cli {
    #[arg(short = 'C', long, global = true, value_name = "DIR", help = "Run as if started in this directory")]
    directory: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand, about = "Manage project configuration and inspect the project overview")]
    Project(ProjectCommand),

    #[command(subcommand, about = "Manage development worktrees")]
    Worktree(WorktreeCommand),

    #[command(subcommand, about = "Build, activate, inspect, and prune generations")]
    Generation(GenerationCommand),

    #[command(subcommand, about = "Inspect and control the service process")]
    Service(ServiceCommand),
}

#[derive(Subcommand)]
enum ProjectCommand {
    #[command(about = "Create generation-manager.toml in the current directory")]
    Init {
        #[arg(long, help = "Language preset: rust, node, python, go, generic")]
        preset: Option<String>,
        #[arg(long, help = "Project name (defaults to the directory name)")]
        name: Option<String>,
        #[arg(long, help = "Overwrite an existing manifest")]
        force: bool,
    },

    #[command(about = "Show the project, active generation, and service state")]
    Status,
}

#[derive(Subcommand)]
enum WorktreeCommand {
    #[command(about = "Create a worktree and a branch for it")]
    Create {
        name: String,
        #[arg(long, help = "Base commit or branch for the new branch")]
        base: Option<String>,
    },
    #[command(about = "List worktrees managed by this project")]
    List,
    #[command(about = "Run a worktree's code directly, without creating a generation")]
    Run {
        #[arg(value_name = "WORKTREE", help = "Worktree to run (defaults to the one you are standing in)")]
        worktree: Option<String>,
        #[arg(long, help = "Skip the build stage before running")]
        no_build: bool,
        #[arg(long, help = "Run in the background instead of attaching to the terminal")]
        detach: bool,
    },
    #[command(about = "Remove a worktree")]
    Remove {
        name: String,
        #[arg(long, help = "Remove even with uncommitted changes")]
        force: bool,
    },
}

#[derive(Subcommand)]
enum GenerationCommand {
    #[command(about = "Build and test a checkout into a new generation")]
    Build {
        #[arg(value_name = "WORKTREE", help = "Build from this worktree (defaults to the current worktree or project root)")]
        worktree: Option<String>,
        #[arg(long, help = "Free-form note stored with the generation")]
        note: Option<String>,
        #[arg(long, help = "Activate the generation immediately after a successful build")]
        activate: bool,
    },

    #[command(about = "List generations")]
    List,

    #[command(about = "Activate a generation and verify it")]
    Activate {
        #[arg(value_name = "GENERATION", help = "Generation number (defaults to the newest)")]
        generation: Option<u64>,
    },

    #[command(about = "Return to an older generation")]
    Rollback {
        #[arg(value_name = "GENERATION", help = "Target generation (defaults to the one before the active)")]
        generation: Option<u64>,
    },

    #[command(about = "Show the activation log")]
    History,

    #[command(about = "Delete old generations")]
    Prune {
        #[arg(long, default_value_t = 5, help = "Number of recent generations to keep")]
        keep: usize,
    },
}

#[derive(Subcommand)]
enum ServiceCommand {
    #[command(about = "Show the service process and its source")]
    Status,
    #[command(about = "Start the service from the active generation")]
    Start,
    #[command(about = "Stop the running service")]
    Stop,
    #[command(about = "Stop then start the active generation")]
    Restart,
    #[command(about = "Show the tail of the service log")]
    Logs {
        #[arg(short = 'n', long, default_value_t = 40)]
        lines: usize,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(dir) = &cli.directory
        && let Err(err) = std::env::set_current_dir(dir)
    {
        eprintln!("{} cannot enter {}: {err}", ui::ERR, dir.display());
        return ExitCode::FAILURE;
    }

    match cmd::dispatch(cli.command) {
        | Ok(code) => code,
        | Err(err) => {
            eprintln!("{} {err:#}", ui::ERR);
            ExitCode::FAILURE
        },
    }
}
