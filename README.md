# IssueCapsule

Turn GitHub issues into reproducible bugs.

IssueCapsule v0.2 uses AI to understand a bug report, validates the proposed
steps, and compiles them into a deterministic `.icap` file.

```text
$ issuecap create https://github.com/foo/bar/issues/123 --ai

Analyzing issue with AI...

✓ Python 3.12
✓ Reproduction code discovered
✓ Expected error: TypeError

Running sandbox...

BUG REPRODUCED ✓
Saved → issue-123.icap
```

AI understands the report. IssueCapsule reproduces it deterministically.

> [!IMPORTANT]
> Git and Docker are required. v0.2 supports public GitHub issues and Python
> projects. AI mode also needs an OpenAI-compatible JSON chat endpoint.

## Install

Install v0.2 from crates.io:

```bash
cargo install issuecapsule --version 0.2.0
```

The command is named `issuecap`. Check the local requirements:

```bash
issuecap doctor
```

Prebuilt Windows, Linux, and macOS archives are available from
[GitHub Releases](https://github.com/wantenhot/IssueCapsule/releases).

Build the current source instead with:

```bash
git clone https://github.com/wantenhot/IssueCapsule.git
cd IssueCapsule
cargo install --path .
```

## Create with AI

Set three environment variables for any compatible HTTP endpoint:

```bash
export ISSUECAP_AI_BASE_URL="https://your-provider.example/v1"
export ISSUECAP_AI_API_KEY="<api-key>"
export ISSUECAP_AI_MODEL="<model-name>"
```

PowerShell:

```powershell
$env:ISSUECAP_AI_BASE_URL = "https://your-provider.example/v1"
$env:ISSUECAP_AI_API_KEY = "<api-key>"
$env:ISSUECAP_AI_MODEL = "<model-name>"
```

Then analyze and reproduce an issue:

```bash
issuecap create https://github.com/foo/bar/issues/123 --ai
```

Before anything runs, IssueCapsule shows the proposed Python version, install
command, generated files, reproduction command, expected error, and confidence.
Confirm the plan to build the Docker sandbox.

Use `--yes` in CI to accept a validated plan with at least 60% confidence:

```bash
issuecap create https://github.com/foo/bar/issues/123 --ai --yes
```

Plans below 60% do not run by default. `--force` lets you review and confirm a
low-confidence plan, but it never bypasses validation.

## Preview a plan

Use `--ai-plan` to analyze an issue without running Docker or creating a
capsule:

```bash
issuecap create https://github.com/foo/bar/issues/123 --ai-plan
```

## Create manually

AI remains optional. Supply the reproduction command and expected output
yourself:

```bash
issuecap create https://github.com/foo/bar/issues/123 \
  --run "python reproduce.py" \
  --expect "IndexError"
```

## Share and verify

Run a saved capsule without an AI key:

```bash
issuecap run issue-123.icap
```

Verify that its expected error still occurs:

```bash
issuecap verify issue-123.icap
```

`verify` exits with code `0` when the bug is reproduced and code `1` when it
is not.

## How AI mode stays controlled

AI is used only during `create`. It receives a limited context: the issue,
up to 200 repository paths, small excerpts from dependency files and README,
and code blocks already present in the issue.

Its response must be strict JSON. IssueCapsule then:

1. Parses it into a structured reproduction plan.
2. Rejects unsafe commands and paths outside the repository.
3. Shows the complete plan for review.
4. Installs dependencies while building the Docker image.
5. Runs the reproduction command with Docker networking disabled.
6. Saves the validated plan and generated files in `.icap`.

The API key stays in the IssueCapsule process. It is not saved in `.icap`,
logs, generated files, Git, or the Docker environment.

> [!WARNING]
> AI output is untrusted input. The validator cannot be bypassed by `--yes` or
> `--force`. You should still review plans and capsules received from others.

## Commands

| Command | Purpose |
| --- | --- |
| `issuecap create URL --ai` | Analyze, review, reproduce, and save a capsule |
| `issuecap create URL --ai-plan` | Print a plan only |
| `issuecap create URL --run CMD --expect TEXT` | Use manual mode |
| `issuecap run FILE.icap` | Run a deterministic capsule |
| `issuecap verify FILE.icap` | Check its expected error |
| `issuecap doctor` | Check Git, Docker, and GitHub access |

## Current limitations

- Public GitHub repositories only
- Python projects only
- Docker required for execution
- OpenAI-compatible HTTP response shape in AI mode
- Dependency detection limited to `requirements.txt` and `pyproject.toml`

IssueCapsule does not fix bugs. It makes bugs reproducible.

## Develop

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

[MIT](LICENSE)
