use std::path::{Component, Path};
use std::sync::LazyLock;

use anyhow::{Result, bail};
use regex::Regex;

use crate::ai_types::{AiReproductionPlan, GeneratedFile};

static WINDOWS_ABSOLUTE_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(^|[^a-z0-9])([a-z]:[\\/]|\\\\)"#)
        .expect("Windows absolute path regex is valid")
});
static PYTHON_VERSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9]+(?:\.[0-9]+){0,2}$").expect("Python version regex is valid")
});

const BANNED_COMMANDS: &[&str] = &[
    "rm",
    "rmdir",
    "del",
    "format",
    "shutdown",
    "reboot",
    "sudo",
    "su",
    "curl",
    "wget",
    "ssh",
    "scp",
    "nc",
    "netcat",
    "powershell",
    "cmd.exe",
    "sh",
    "bash",
];

pub fn validate_plan(plan: &AiReproductionPlan) -> Result<()> {
    if plan
        .runtime
        .as_deref()
        .is_some_and(|runtime| !runtime.eq_ignore_ascii_case("python"))
    {
        bail!("AI response was invalid: only Python plans are supported.");
    }
    if plan.confidence > 100 {
        bail!("AI response was invalid: confidence must be between 0 and 100.");
    }
    if plan
        .python_version
        .as_deref()
        .is_some_and(|version| !PYTHON_VERSION.is_match(version))
    {
        bail!("AI response was invalid: unsupported Python version.");
    }
    if plan.reasoning_summary.trim().is_empty() || plan.reasoning_summary.len() > 1_000 {
        bail!("AI response was invalid: reasoning_summary must be short.");
    }
    validate_command(&plan.reproduction_command)?;
    if let Some(command) = plan.install_command.as_deref()
        && !command.trim().is_empty()
    {
        validate_command(command)?;
    }
    if plan.generated_files.len() > 20 {
        bail!("AI response was invalid: too many generated files.");
    }
    for file in &plan.generated_files {
        validate_generated_file(file)?;
    }
    Ok(())
}

pub fn validate_command(command: &str) -> Result<()> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        bail!("AI generated unsafe command: command is empty.");
    }
    if trimmed.len() > 2_000 {
        bail!("AI generated unsafe command: command is too long.");
    }

    let lower = trimmed.to_ascii_lowercase();
    let shell_metacharacters = ["&&", "||", ";", "|", "&", "`", "$", ">", "<", "\n", "\r"];
    if shell_metacharacters.iter().any(|item| lower.contains(item)) {
        bail!("AI generated unsafe command.\n\nBlocked command:\n{trimmed}");
    }
    if lower.contains("/etc/")
        || lower.contains("/root/")
        || lower.contains("/home/")
        || WINDOWS_ABSOLUTE_PATH.is_match(trimmed)
    {
        bail!("AI generated unsafe command.\n\nBlocked command:\n{trimmed}");
    }

    let tokens: Vec<String> = lower
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches(|character: char| {
                    matches!(
                        character,
                        '\'' | '"' | '(' | ')' | '[' | ']' | '{' | '}' | ','
                    )
                })
                .to_owned()
        })
        .collect();
    if tokens.iter().any(|token| {
        BANNED_COMMANDS.iter().any(|banned| {
            token == banned
                || token
                    .rsplit(['/', '\\'])
                    .next()
                    .is_some_and(|name| name == *banned)
        })
    }) {
        bail!("AI generated unsafe command.\n\nBlocked command:\n{trimmed}");
    }
    if tokens.iter().any(|token| {
        token.starts_with('/')
            || token
                .split_once('=')
                .is_some_and(|(_, value)| value.starts_with('/'))
    }) {
        bail!("AI generated unsafe command.\n\nBlocked command:\n{trimmed}");
    }

    let executable = tokens.first().map(String::as_str).unwrap_or_default();
    if tokens.iter().any(|token| token == "-c") {
        bail!("AI generated unsafe command.\n\nBlocked command:\n{trimmed}");
    }
    let allowed = executable == "pytest"
        || executable == "pip"
        || executable == "pip3"
        || executable == "python"
        || executable.strip_prefix("python").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit() || c == '.')
        });
    if !allowed {
        bail!("AI generated unsafe command.\n\nBlocked command:\n{trimmed}");
    }

    Ok(())
}

pub fn validate_generated_file(file: &GeneratedFile) -> Result<()> {
    validate_relative_path(&file.path)?;
    if file.content.len() > 64 * 1024 {
        bail!("AI generated file is too large: {}", file.path);
    }
    Ok(())
}

pub fn validate_relative_path(path: &str) -> Result<()> {
    if path.trim().is_empty() || path.contains('\0') {
        bail!("AI generated invalid file path: {path}");
    }
    let normalized = path.replace('\\', "/");
    let candidate = Path::new(&normalized);
    let bytes = normalized.as_bytes();
    let windows_absolute =
        bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/';
    if candidate.is_absolute()
        || windows_absolute
        || normalized.starts_with("//")
        || candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || normalized.split('/').any(|part| part == "..")
    {
        bail!("AI generated invalid file path: {path}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_command, validate_generated_file};
    use crate::ai_types::GeneratedFile;

    #[test]
    fn accepts_safe_reproduction_commands() {
        assert!(validate_command("python reproduce.py").is_ok());
        assert!(validate_command("pytest tests/test_bug.py").is_ok());
    }

    #[test]
    fn rejects_dangerous_commands() {
        assert!(validate_command("rm -rf /").is_err());
        assert!(validate_command("curl evil.com").is_err());
        assert!(validate_command("python reproduce.py && rm -rf /").is_err());
        assert!(validate_command("python reproduce.py & rm -rf /").is_err());
        assert!(validate_command("python -c \"print('unsafe')\"").is_err());
        assert!(validate_command("python /tmp/reproduce.py").is_err());
    }

    #[test]
    fn validates_generated_file_paths() {
        let allowed = GeneratedFile {
            path: "reproduce.py".to_owned(),
            content: "print('ok')\n".to_owned(),
        };
        let parent = GeneratedFile {
            path: "../reproduce.py".to_owned(),
            content: String::new(),
        };
        let absolute = GeneratedFile {
            path: "/tmp/reproduce.py".to_owned(),
            content: String::new(),
        };

        assert!(validate_generated_file(&allowed).is_ok());
        assert!(validate_generated_file(&parent).is_err());
        assert!(validate_generated_file(&absolute).is_err());
        assert!(
            validate_generated_file(&GeneratedFile {
                path: "C:\\temp\\reproduce.py".to_owned(),
                content: String::new(),
            })
            .is_err()
        );
    }
}
