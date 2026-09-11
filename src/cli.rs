use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

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
        /// Command that reproduces the bug.
        #[arg(long)]
        run: String,
        /// Text expected in stdout or stderr.
        #[arg(long)]
        expect: String,
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

pub fn create(issue_url: &str, run: &str, expect: &str) -> Result<()> {
    if run.trim().is_empty() {
        bail!("Reproduction command cannot be empty.");
    }
    if expect.is_empty() {
        bail!("Expected error text cannot be empty.");
    }

    let issue = crate::github::parse_issue_url(issue_url)?;
    println!("IssueCapsule\n");
    println!("Fetching issue...");
    let info = crate::github::fetch_issue(&issue)?;
    println!("✓ #{} {}", issue.number, info.title);
    println!("\nCloning repository...");

    let temporary = tempfile::tempdir()?;
    let repository = temporary.path().join("repository");
    crate::git::clone_repository(&issue.repository_url(), &repository)?;
    println!("✓ {}", issue.slug());

    let commit = crate::git::head_commit(&repository)?;
    let project = crate::detector::PythonProject::detect(&repository);
    println!("\nCommit:\n{}", &commit[..commit.len().min(7)]);
    println!("\nEnvironment:\nPython\n{}", project.label());

    let capsule = crate::capsule::Capsule::new(
        issue_url,
        issue.number,
        &info.title,
        &issue.repository_url(),
        &commit,
        project,
        run,
        expect,
    );

    println!("\nBuilding sandbox...");
    let execution = crate::docker::execute(&repository, &capsule)?;
    execution.print();
    println!("\n---\n");

    if !execution.contains(expect) {
        println!("BUG NOT REPRODUCED ✗");
        bail!("Bug could not be reproduced.");
    }

    println!("BUG REPRODUCED ✓");
    let output_path = PathBuf::from(format!("issue-{}.icap", issue.number));
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
    println!("Building sandbox...");
    let execution = crate::docker::execute(&repository, &capsule)?;

    Ok((capsule, execution))
}
