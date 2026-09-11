use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::ai_types::{AiReproductionPlan, GeneratedFile};
use crate::detector::PythonProject;

static COMMIT_SHA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-fA-F]{7,64}$").expect("commit SHA regex is valid"));
static PYTHON_VERSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9]+(?:\.[0-9]+){0,2}$").expect("Python version regex is valid")
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capsule {
    pub version: u32,
    pub issue: Issue,
    pub source: Source,
    pub environment: Environment,
    pub install: Install,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiMetadata>,
    pub reproduce: Reproduce,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<GeneratedFile>,
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
pub struct AiMetadata {
    pub generated: bool,
    pub model: String,
    pub confidence: u8,
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
            version: 2,
            issue: issue(issue_url, issue_number, issue_title),
            source: source(repository, commit),
            environment: Environment {
                language: "python".to_owned(),
                docker_image: "python:3.12-slim".to_owned(),
            },
            install: Install {
                strategy: project.strategy().to_owned(),
                command: project.install_command().to_owned(),
            },
            ai: None,
            reproduce: Reproduce {
                command: command.to_owned(),
                expected_error: expected_error.to_owned(),
            },
            files: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_ai_plan(
        issue_url: &str,
        issue_number: u64,
        issue_title: &str,
        repository: &str,
        commit: &str,
        project: PythonProject,
        model: &str,
        plan: AiReproductionPlan,
    ) -> Result<Self> {
        crate::validator::validate_plan(&plan)?;
        let expected_error = plan
            .expected_error
            .as_deref()
            .filter(|value| !value.is_empty())
            .context("AI response was invalid: expected_error is required to reproduce the bug.")?;
        let python_version = plan.python_version.as_deref().unwrap_or("3.12");
        if !PYTHON_VERSION.is_match(python_version) {
            bail!("AI response was invalid: unsupported Python version.");
        }

        let install_command = plan
            .install_command
            .as_deref()
            .unwrap_or_else(|| project.install_command());
        let strategy = if plan.install_command.is_some() {
            "ai-plan"
        } else {
            project.strategy()
        };

        Ok(Self {
            version: 2,
            issue: issue(issue_url, issue_number, issue_title),
            source: source(repository, commit),
            environment: Environment {
                language: "python".to_owned(),
                docker_image: format!("python:{python_version}-slim"),
            },
            install: Install {
                strategy: strategy.to_owned(),
                command: install_command.to_owned(),
            },
            ai: Some(AiMetadata {
                generated: true,
                model: model.to_owned(),
                confidence: plan.confidence,
            }),
            reproduce: Reproduce {
                command: plan.reproduction_command,
                expected_error: expected_error.to_owned(),
            },
            files: plan.generated_files,
        })
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

    pub fn materialize_files(&self, repository: &Path) -> Result<()> {
        for file in &self.files {
            crate::validator::validate_generated_file(file)?;
            let destination = safe_destination(repository, &file.path)?;
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create generated file directory: {}", file.path)
                })?;
            }
            fs::write(&destination, &file.content)
                .with_context(|| format!("Failed to write generated file: {}", file.path))?;
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 && self.version != 2 {
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
        for file in &self.files {
            crate::validator::validate_generated_file(file)?;
        }
        if let Some(ai) = &self.ai {
            if !ai.generated || ai.confidence > 100 {
                bail!("Invalid AI capsule metadata.");
            }
            if !self.install.command.is_empty() {
                crate::validator::validate_command(&self.install.command)?;
            }
            crate::validator::validate_command(&self.reproduce.command)?;
        }
        Ok(())
    }
}

fn issue(url: &str, number: u64, title: &str) -> Issue {
    Issue {
        url: url.to_owned(),
        number,
        title: title.to_owned(),
    }
}

fn source(repository: &str, commit: &str) -> Source {
    Source {
        repository: repository.to_owned(),
        commit: commit.to_owned(),
    }
}

fn safe_destination(repository: &Path, relative: &str) -> Result<PathBuf> {
    crate::validator::validate_relative_path(relative)?;
    let mut current = repository.to_path_buf();
    for part in relative.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        current.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&current)
            && metadata.file_type().is_symlink()
        {
            bail!("AI generated invalid file path: {relative}");
        }
    }
    if current.exists() {
        bail!("AI generated file already exists in the repository: {relative}");
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::Capsule;
    use crate::ai_types::{AiReproductionPlan, GeneratedFile};
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
    fn serializes_capsule_v2_to_toml() {
        let plan = AiReproductionPlan {
            runtime: Some("python".to_owned()),
            python_version: Some("3.12".to_owned()),
            install_command: Some("pip install .".to_owned()),
            reproduction_command: "python reproduce.py".to_owned(),
            expected_error: Some("TypeError".to_owned()),
            generated_files: vec![GeneratedFile {
                path: "reproduce.py".to_owned(),
                content: "print('test')\n".to_owned(),
            }],
            confidence: 88,
            reasoning_summary: "Minimal example in issue.".to_owned(),
        };
        let capsule = Capsule::from_ai_plan(
            "https://github.com/foo/bar/issues/123",
            123,
            "Crash",
            "https://github.com/foo/bar.git",
            "0123456789abcdef0123456789abcdef01234567",
            PythonProject::Pyproject,
            "test-model",
            plan,
        )
        .unwrap();
        let serialized = toml::to_string_pretty(&capsule).unwrap();

        assert!(serialized.contains("version = 2"));
        assert!(serialized.contains("[ai]"));
        assert!(serialized.contains("confidence = 88"));
        assert!(serialized.contains("[[files]]"));
        let deserialized: Capsule = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized, capsule);
    }

    #[test]
    fn deserializes_legacy_v1_capsule() {
        let mut capsule = example();
        capsule.version = 1;
        let serialized = toml::to_string_pretty(&capsule).unwrap();
        let deserialized: Capsule = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized, capsule);
    }

    #[test]
    fn materializes_new_files_without_overwriting_repository_files() {
        let directory = tempdir().unwrap();
        let mut capsule = example();
        capsule.files.push(GeneratedFile {
            path: "tests/reproduce.py".to_owned(),
            content: "raise TypeError('expected')\n".to_owned(),
        });

        capsule.materialize_files(directory.path()).unwrap();
        assert_eq!(
            fs::read_to_string(directory.path().join("tests/reproduce.py")).unwrap(),
            "raise TypeError('expected')\n"
        );
        assert!(capsule.materialize_files(directory.path()).is_err());
    }
}
