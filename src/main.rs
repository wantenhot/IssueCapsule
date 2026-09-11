mod ai;
mod ai_types;
mod capsule;
mod cli;
mod detector;
mod docker;
mod doctor;
mod git;
mod github;
mod validator;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Create {
            issue_url,
            run,
            expect,
            ai,
            ai_plan,
            yes,
            force,
        } => cli::create(cli::CreateOptions {
            issue_url: &issue_url,
            run: run.as_deref(),
            expect: expect.as_deref(),
            ai,
            ai_plan,
            yes,
            force,
        }),
        Command::Run { capsule } => cli::run_capsule(&capsule),
        Command::Verify { capsule } => cli::verify_capsule(&capsule),
        Command::Doctor => doctor::run(),
    }
}
