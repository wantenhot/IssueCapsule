use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PythonProject {
    Requirements,
    Pyproject,
    NoDependencies,
}

impl PythonProject {
    pub fn detect(repository: &Path) -> Self {
        if repository.join("requirements.txt").is_file() {
            Self::Requirements
        } else if repository.join("pyproject.toml").is_file() {
            Self::Pyproject
        } else {
            Self::NoDependencies
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Requirements => "requirements.txt",
            Self::Pyproject => "pyproject.toml",
            Self::NoDependencies => "no dependency file",
        }
    }

    pub fn strategy(self) -> &'static str {
        match self {
            Self::Requirements => "requirements",
            Self::Pyproject => "pyproject",
            Self::NoDependencies => "none",
        }
    }

    pub fn install_command(self) -> &'static str {
        match self {
            Self::Requirements => "pip install -r requirements.txt",
            Self::Pyproject => "pip install .",
            Self::NoDependencies => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::PythonProject;

    #[test]
    fn detects_requirements_first() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("requirements.txt"), "").unwrap();
        fs::write(directory.path().join("pyproject.toml"), "").unwrap();

        assert_eq!(
            PythonProject::detect(directory.path()),
            PythonProject::Requirements
        );
    }

    #[test]
    fn detects_pyproject() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("pyproject.toml"), "").unwrap();

        assert_eq!(
            PythonProject::detect(directory.path()),
            PythonProject::Pyproject
        );
    }

    #[test]
    fn detects_no_dependency_file() {
        let directory = tempdir().unwrap();

        assert_eq!(
            PythonProject::detect(directory.path()),
            PythonProject::NoDependencies
        );
    }
}
