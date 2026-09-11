use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::detector::PythonProject;

static COMMIT_SHA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-fA-F]{7,64}$").expect("commit SHA regex is valid"));

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capsule {
    pub version: u32,
    pub issue: Issue,
    pub source: Source,
    pub environment: Environment,
    pub install: Install,
    pub reproduce: Reproduce,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub url: String,
    pub number: u64,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub repository: String,
    pub commit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub language: String,
    pub docker_image: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Install {
    pub strategy: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reproduce {
    pub command: String,
    pub expected_error: String,
}

impl Capsule {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        issue_url: &str,
        issue_number: u64,
        issue_title: &str,
        repository: &str,
        commit: &str,
        project: PythonProject,
        command: &str,
        expected_error: &str,
    ) -> Self {
        Self {
            version: 1,
            issue: Issue {
                url: issue_url.to_owned(),
                number: issue_number,
                title: issue_title.to_owned(),
            },
            source: Source {
                repository: repository.to_owned(),
                commit: commit.to_owned(),
            },
            environment: Environment {
                language: "python".to_owned(),
                docker_image: "python:3.12-slim".to_owned(),
            },
            install: Install {
                strategy: project.strategy().to_owned(),
                command: project.install_command().to_owned(),
            },
            reproduce: Reproduce {
                command: command.to_owned(),
                expected_error: expected_error.to_owned(),
            },
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read capsule: {}", path.display()))?;
        let capsule: Self = toml::from_str(&contents).context("Invalid capsule TOML.")?;
        capsule.validate()?;
        Ok(capsule)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let contents = toml::to_string_pretty(self).context("Failed to serialize capsule.")?;
        fs::write(path, contents)
            .with_context(|| format!("Failed to save capsule: {}", path.display()))
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 {
            bail!("Unsupported capsule version: {}.", self.version);
        }
        if self.environment.language != "python" {
            bail!("Only Python capsules are supported.");
        }
        if !self.environment.docker_image.starts_with("python:")
            || !self
                .environment
                .docker_image
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".:/@_-".contains(character))
        {
            bail!("Invalid Python Docker image.");
        }
        if !self.source.repository.starts_with("https://github.com/")
            || !self.source.repository.ends_with(".git")
        {
            bail!("Capsule repository must be a public GitHub repository.");
        }
        if !COMMIT_SHA.is_match(&self.source.commit) {
            bail!("Invalid capsule commit SHA.");
        }
        if self.reproduce.command.trim().is_empty() {
            bail!("Reproduction command cannot be empty.");
        }
        if self.reproduce.expected_error.is_empty() {
            bail!("Expected error text cannot be empty.");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Capsule;
    use crate::detector::PythonProject;

    fn example() -> Capsule {
        Capsule::new(
            "https://github.com/foo/bar/issues/123",
            123,
            "Crash when input is empty",
            "https://github.com/foo/bar.git",
            "0123456789abcdef0123456789abcdef01234567",
            PythonProject::Requirements,
            "python reproduce.py",
            "IndexError",
        )
    }

    #[test]
    fn serializes_capsule_to_toml() {
        let serialized = toml::to_string_pretty(&example()).unwrap();

        assert!(serialized.contains("version = 1"));
        assert!(serialized.contains("[reproduce]"));
        assert!(serialized.contains("expected_error = \"IndexError\""));
    }

    #[test]
    fn deserializes_capsule_from_toml() {
        let capsule = example();
        let serialized = toml::to_string_pretty(&capsule).unwrap();
        let deserialized: Capsule = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized, capsule);
    }
}
