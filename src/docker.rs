use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

use crate::capsule::Capsule;

pub struct Execution {
    pub stdout: String,
    pub stderr: String,
    exit_code: Option<i32>,
}

impl Execution {
    pub fn contains(&self, expected: &str) -> bool {
        self.stdout.contains(expected) || self.stderr.contains(expected)
    }

    pub fn print(&self) {
        print!("{}", self.stdout);
        print!("{}", self.stderr);
        if self.exit_code.is_none() {
            println!("Process terminated without an exit code.");
        }
    }
}

struct DockerImage(String);

impl Drop for DockerImage {
    fn drop(&mut self) {
        let _ = command().args(["image", "rm"]).arg(&self.0).output();
    }
}

pub fn is_available() -> bool {
    command()
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

pub fn daemon_available() -> bool {
    command()
        .arg("info")
        .output()
        .is_ok_and(|output| output.status.success())
}

pub fn ensure_available() -> Result<()> {
    match command().arg("--version").output() {
        Ok(output) if output.status.success() => {}
        Ok(_) => bail!("Docker is required.\nRun `issuecap doctor`."),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            bail!("Docker is required.\nRun `issuecap doctor`.")
        }
        Err(error) => return Err(error).context("Could not check Docker."),
    }

    if !daemon_available() {
        bail!("Docker is installed but the daemon is not running.");
    }

    Ok(())
}

pub fn execute(repository: &Path, capsule: &Capsule) -> Result<Execution> {
    ensure_available()?;
    let image = build(repository, capsule)?;
    println!("✓ Docker image ready");
    println!("\nRunning:\n\n{}\n\n---\n", capsule.reproduce.command);
    let output = command()
        .args(["run", "--rm", "--network", "none"])
        .arg(&image.0)
        .args(["sh", "-lc"])
        .arg(&capsule.reproduce.command)
        .output()
        .context("Failed to run reproduction command in Docker.")?;

    Ok(execution_from(output))
}

fn build(repository: &Path, capsule: &Capsule) -> Result<DockerImage> {
    let directory = tempfile::tempdir().context("Failed to create Docker build files.")?;
    let dockerfile = directory.path().join("Dockerfile");
    let contents = dockerfile_contents(capsule);
    fs::write(&dockerfile, contents).context("Failed to write temporary Dockerfile.")?;

    let output = command()
        .args(["build", "--quiet", "--file"])
        .arg(&dockerfile)
        .arg(repository)
        .output()
        .context("Failed to start Docker build.")?;

    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to build reproduction environment.\n{details}");
    }

    let stdout =
        String::from_utf8(output.stdout).context("Docker returned an invalid image ID.")?;
    let image_id = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .ok_or_else(|| anyhow::anyhow!("Docker did not return a built image ID."))?;

    Ok(DockerImage(image_id.to_owned()))
}

fn execution_from(output: Output) -> Execution {
    Execution {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code(),
    }
}

fn command() -> Command {
    let mut command = Command::new("docker");
    command
        .env_remove("ISSUECAP_AI_API_KEY")
        .env_remove("ISSUECAP_AI_BASE_URL")
        .env_remove("ISSUECAP_AI_MODEL");
    command
}

fn dockerfile_contents(capsule: &Capsule) -> String {
    let mut contents = format!(
        "FROM {}\nWORKDIR /app\nCOPY . .\nRUN python -m pip install --upgrade pip\n",
        capsule.environment.docker_image
    );
    if !capsule.install.command.is_empty() {
        contents.push_str("RUN ");
        contents.push_str(&capsule.install.command);
        contents.push('\n');
    }
    contents.push_str("CMD [\"sh\"]\n");
    contents
}

#[cfg(test)]
mod tests {
    use crate::capsule::Capsule;
    use crate::detector::PythonProject;

    use super::dockerfile_contents;

    #[test]
    fn dockerfile_installs_requirements() {
        let capsule = example(PythonProject::Requirements);

        assert!(dockerfile_contents(&capsule).contains("RUN pip install -r requirements.txt\n"));
    }

    #[test]
    fn dockerfile_skips_empty_install_command() {
        let capsule = example(PythonProject::NoDependencies);

        assert!(!dockerfile_contents(&capsule).contains("RUN \n"));
    }

    fn example(project: PythonProject) -> Capsule {
        Capsule::new(
            "https://github.com/foo/bar/issues/123",
            123,
            "Example",
            "https://github.com/foo/bar.git",
            "0123456789abcdef0123456789abcdef01234567",
            project,
            "python reproduce.py",
            "IndexError",
        )
    }
}
