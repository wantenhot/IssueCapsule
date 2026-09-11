use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn is_available() -> bool {
    command()
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

pub fn ensure_available() -> Result<()> {
    match command().arg("--version").output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) => bail!("Git is required.\nRun `issuecap doctor`."),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            bail!("Git is required.\nRun `issuecap doctor`.")
        }
        Err(error) => Err(error).context("Could not check Git."),
    }
}

pub fn clone_repository(repository: &str, destination: &Path) -> Result<()> {
    ensure_available()?;
    let output = command()
        .args(["clone", "--quiet", repository])
        .arg(destination)
        .output()
        .context("Failed to start Git.")?;

    if !output.status.success() {
        bail!("Failed to clone repository.");
    }

    Ok(())
}

pub fn checkout(repository: &Path, commit: &str) -> Result<()> {
    let output = command()
        .args(["-C"])
        .arg(repository)
        .args(["checkout", "--quiet", commit])
        .output()
        .context("Failed to start Git.")?;

    if !output.status.success() {
        bail!("Failed to check out capsule commit.");
    }

    Ok(())
}

pub fn head_commit(repository: &Path) -> Result<String> {
    let output = command()
        .args(["-C"])
        .arg(repository)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("Failed to read repository commit.")?;

    if !output.status.success() {
        bail!("Failed to read repository commit.");
    }

    let commit = String::from_utf8(output.stdout).context("Git returned an invalid commit SHA.")?;
    Ok(commit.trim().to_owned())
}

fn command() -> Command {
    let mut command = Command::new("git");
    command
        .env_remove("ISSUECAP_AI_API_KEY")
        .env_remove("ISSUECAP_AI_BASE_URL")
        .env_remove("ISSUECAP_AI_MODEL");
    command
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use tempfile::tempdir;

    use super::{clone_repository, head_commit, is_available};

    #[test]
    fn clones_repository_and_reads_head() {
        if !is_available() {
            return;
        }

        let temporary = tempdir().unwrap();
        let source = temporary.path().join("source");
        let clone = temporary.path().join("clone");
        fs::create_dir(&source).unwrap();
        run_git(&source, &["init", "--quiet"]);
        run_git(&source, &["config", "user.name", "IssueCapsule Test"]);
        run_git(
            &source,
            &["config", "user.email", "issuecapsule@example.invalid"],
        );
        fs::write(source.join("README.md"), "test repository\n").unwrap();
        run_git(&source, &["add", "README.md"]);
        run_git(&source, &["commit", "--quiet", "-m", "Initial commit"]);
        let expected = head_commit(&source).unwrap();

        clone_repository(source.to_str().unwrap(), &clone).unwrap();

        assert_eq!(head_commit(&clone).unwrap(), expected);
    }

    fn run_git(repository: &std::path::Path, arguments: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success());
    }
}
