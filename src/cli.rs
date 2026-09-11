use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};

use crate::ai_types::AiReproductionPlan;

#[derive(Debug, Parser)]
#[command(name = "issuecap", version, about = "Make GitHub bugs reproducible")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a capsule from a public GitHub issue.
    Create {
        /// GitHub issue URL.
        issue_url: String,
        /// Command that reproduces the bug (manual mode).
        #[arg(long)]
        run: Option<String>,
        /// Text expected in stdout or stderr (manual mode).
        #[arg(long)]
        expect: Option<String>,
        /// Ask an AI service for a structured reproduction plan.
        #[arg(long, conflicts_with = "ai_plan")]
        ai: bool,
        /// Print an AI plan without running Docker or creating a capsule.
        #[arg(long, conflicts_with = "ai")]
        ai_plan: bool,
        /// Continue without prompting when AI confidence is at least 60%.
        #[arg(long, requires = "ai")]
        yes: bool,
        /// Allow a low-confidence plan to reach the confirmation prompt.
        #[arg(long, requires = "ai")]
        force: bool,
    },
    /// Re-run a capsule and print its output.
    Run {
        /// Path to a .icap file.
        capsule: PathBuf,
    },
    /// Re-run a capsule and verify its expected error.
    Verify {
        /// Path to a .icap file.
        capsule: PathBuf,
    },
    /// Check required local tools and GitHub connectivity.
    Doctor,
}

pub struct CreateOptions<'a> {
    pub issue_url: &'a str,
    pub run: Option<&'a str>,
    pub expect: Option<&'a str>,
    pub ai: bool,
    pub ai_plan: bool,
    pub yes: bool,
    pub force: bool,
}

pub fn create(options: CreateOptions<'_>) -> Result<()> {
    let ai_requested = options.ai || options.ai_plan;
    if ai_requested && (options.run.is_some() || options.expect.is_some()) {
        bail!("--run and --expect cannot be combined with --ai or --ai-plan.");
    }
    if !ai_requested {
        let run = options
            .run
            .filter(|value| !value.trim().is_empty())
            .context("Manual mode requires --run.")?;
        let expect = options
            .expect
            .filter(|value| !value.is_empty())
            .context("Manual mode requires --expect.")?;
        return create_manual(options.issue_url, run, expect);
    }

    create_with_ai(options)
}

fn create_manual(issue_url: &str, run: &str, expect: &str) -> Result<()> {
    let prepared = prepare_repository(issue_url)?;
    print_repository_summary(&prepared);
    let capsule = crate::capsule::Capsule::new(
        issue_url,
        prepared.issue.number,
        &prepared.info.title,
        &prepared.issue.repository_url(),
        &prepared.commit,
        prepared.project,
        run,
        expect,
    );
    reproduce_and_save(&prepared.repository, &capsule)
}

fn create_with_ai(options: CreateOptions<'_>) -> Result<()> {
    let config = crate::ai::AiConfig::from_env()?;
    let prepared = prepare_repository(options.issue_url)?;
    print_repository_summary(&prepared);
    println!("\nAnalyzing issue with AI...");
    let context = crate::ai::collect_context(
        &prepared.repository,
        &prepared.issue.slug(),
        prepared.project,
        &prepared.info.title,
        prepared.info.body.as_deref(),
    )?;
    let plan = crate::ai::analyze_issue(&config, &context).map_err(|error| {
        anyhow!(
            "AI analysis failed.\n{error}\n\nYou can still create the capsule manually:\nissuecap create URL --run \"...\" --expect \"...\""
        )
    })?;
    crate::validator::validate_plan(&plan)?;
    print_plan(&plan);

    if options.ai_plan {
        return Ok(());
    }
    if plan.confidence < 60 && !options.force {
        bail!(
            "AI confidence is low: {}%\n\nThe generated plan may be unreliable.\n\nUse:\nissuecap create {} --ai --force\n\nto review and continue.",
            plan.confidence,
            options.issue_url
        );
    }
    let should_continue = options.yes && plan.confidence >= 60 || confirm()?;
    if !should_continue {
        println!("Cancelled. No capsule was created.");
        return Ok(());
    }

    let capsule = crate::capsule::Capsule::from_ai_plan(
        options.issue_url,
        prepared.issue.number,
        &prepared.info.title,
        &prepared.issue.repository_url(),
        &prepared.commit,
        prepared.project,
        &config.model,
        plan,
    )?;
    reproduce_and_save(&prepared.repository, &capsule)
}

struct PreparedRepository {
    _temporary: tempfile::TempDir,
    repository: PathBuf,
    issue: crate::github::IssueRef,
    info: crate::github::IssueInfo,
    commit: String,
    project: crate::detector::PythonProject,
}

fn prepare_repository(issue_url: &str) -> Result<PreparedRepository> {
    let issue = crate::github::parse_issue_url(issue_url)?;
    println!("IssueCapsule\n");
    println!("Fetching issue...");
    let info = crate::github::fetch_issue(&issue)?;
    println!("✓ #{} {}", issue.number, info.title);
    println!("\nInspecting repository...");

    let temporary = tempfile::tempdir()?;
    let repository = temporary.path().join("repository");
    crate::git::clone_repository(&issue.repository_url(), &repository)?;
    let commit = crate::git::head_commit(&repository)?;
    let project = crate::detector::PythonProject::detect(&repository);
    println!("✓ {}", issue.slug());
    println!("✓ Python project");
    println!("✓ {}", project.label());

    Ok(PreparedRepository {
        _temporary: temporary,
        repository,
        issue,
        info,
        commit,
        project,
    })
}

fn print_repository_summary(prepared: &PreparedRepository) {
    println!(
        "\nCommit:\n{}",
        &prepared.commit[..prepared.commit.len().min(7)]
    );
    println!("\nEnvironment:\nPython\n{}", prepared.project.label());
}

fn print_plan(plan: &AiReproductionPlan) {
    println!("\nAI Reproduction Plan\n");
    println!(
        "Language:\n{}\n",
        plan.runtime.as_deref().unwrap_or("Unknown")
    );
    println!(
        "Python:\n{}\n",
        plan.python_version.as_deref().unwrap_or("Unknown")
    );
    println!(
        "Install:\n{}\n",
        plan.install_command.as_deref().unwrap_or("None")
    );
    println!("Files:");
    if plan.generated_files.is_empty() {
        println!("None\n");
    } else {
        for file in &plan.generated_files {
            println!("\n## {}\n\n{}", file.path, file.content);
        }
    }
    println!("Run:\n{}\n", plan.reproduction_command);
    println!(
        "Expected:\n{}\n",
        plan.expected_error.as_deref().unwrap_or("Unknown")
    );
    println!("Confidence:\n{}%\n", plan.confidence);
    println!("Why:\n{}", plan.reasoning_summary);
}

fn confirm() -> Result<bool> {
    print!("\nContinue with reproduction? [Y/n] ");
    io::stdout().flush().context("Failed to write prompt.")?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("Failed to read confirmation.")?;
    match answer.trim().to_ascii_lowercase().as_str() {
        "" | "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        _ => bail!("Please answer yes or no."),
    }
}

fn reproduce_and_save(repository: &Path, capsule: &crate::capsule::Capsule) -> Result<()> {
    capsule.materialize_files(repository)?;
    println!("\nBuilding sandbox...");
    let execution = crate::docker::execute(repository, capsule)?;
    execution.print();
    println!("\n---\n");

    if !execution.contains(&capsule.reproduce.expected_error) {
        println!("BUG NOT REPRODUCED ✗");
        bail!("Bug could not be reproduced.");
    }

    println!("BUG REPRODUCED ✓");
    let output_path = PathBuf::from(format!("issue-{}.icap", capsule.issue.number));
    capsule.save(&output_path)?;
    println!("\nSaved:\n{}", output_path.display());
    Ok(())
}

pub fn run_capsule(path: &Path) -> Result<()> {
    let (_, execution) = execute_capsule(path)?;
    execution.print();
    println!("\n---");
    Ok(())
}

pub fn verify_capsule(path: &Path) -> Result<()> {
    let (capsule, execution) = execute_capsule(path)?;
    execution.print();
    println!("\n---");

    if execution.contains(&capsule.reproduce.expected_error) {
        println!("\nBUG REPRODUCED ✓");
        Ok(())
    } else {
        println!("\nBUG NOT REPRODUCED ✗");
        bail!("Bug could not be reproduced.")
    }
}

fn execute_capsule(path: &Path) -> Result<(crate::capsule::Capsule, crate::docker::Execution)> {
    let capsule = crate::capsule::Capsule::load(path)?;
    let temporary = tempfile::tempdir()?;
    let repository = temporary.path().join("repository");

    println!("Cloning repository...");
    crate::git::clone_repository(&capsule.source.repository, &repository)?;
    crate::git::checkout(&repository, &capsule.source.commit)?;
    capsule.materialize_files(&repository)?;
    println!("Building sandbox...");
    let execution = crate::docker::execute(&repository, &capsule)?;

    Ok((capsule, execution))
}
