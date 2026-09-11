# IssueCapsule

Turn "works on my machine" into a reproducible bug.

IssueCapsule packages a public GitHub issue, an exact Git commit, and a
reproduction command into a small, editable `.icap` file.

> [!IMPORTANT]
> IssueCapsule requires Git and Docker. It currently supports public GitHub
> repositories and Python projects only.

## Install

With Rust installed:

```bash
cargo install issuecapsule
```

The installed command is `issuecap`. Check that Git, Docker, and GitHub are
available:

```bash
issuecap doctor
```

Prefer a ready-made binary? Download the archive for Windows, Linux, or macOS
from [GitHub Releases](https://github.com/wantenhot/IssueCapsule/releases),
extract it, and place `issuecap` (or `issuecap.exe`) somewhere in your `PATH`.

## Create a reproducible bug

Give IssueCapsule three things:

1. A public GitHub issue URL
2. The command that reproduces the bug
3. Text that should appear in stdout or stderr

```bash
issuecap create https://github.com/foo/bar/issues/123 --run "python reproduce.py" --expect "IndexError"
```

When the error is found, IssueCapsule saves `issue-123.icap`:

```text
BUG REPRODUCED ✓
Saved:
issue-123.icap
```

Share that file with someone else. They can reproduce the same bug with:

```bash
issuecap run issue-123.icap
```

Or verify that the expected error still occurs:

```bash
issuecap verify issue-123.icap
```

`verify` exits with code `0` when the bug is reproduced and code `1` when it
is not.

## How it works

IssueCapsule:

1. Fetches the GitHub issue
2. Clones the repository at an exact commit
3. Detects `requirements.txt` or `pyproject.toml`
4. Builds a clean `python:3.12-slim` Docker image
5. Runs your reproduction command
6. Checks stdout and stderr for the expected text

An `.icap` file is plain TOML. It is not compressed, uploaded, or stored in a
database.

## Commands

| Command | Purpose |
| --- | --- |
| `issuecap create URL --run CMD --expect TEXT` | Reproduce a bug and create a capsule |
| `issuecap run FILE.icap` | Run a capsule and print its output |
| `issuecap verify FILE.icap` | Check whether the recorded error still occurs |
| `issuecap doctor` | Check Git, Docker, and GitHub access |

## Current limitations

- Public GitHub repositories only
- Python projects only
- Docker required
- Reproduction commands must be provided manually
- Dependency detection is limited to `requirements.txt` and `pyproject.toml`

IssueCapsule does not fix bugs. It makes bugs reproducible.

> [!WARNING]
> Capsules contain commands that run in Docker. Review `.icap` files from
> untrusted sources before running them.

## Build from source

```bash
git clone https://github.com/wantenhot/IssueCapsule.git
cd IssueCapsule
cargo build --release
```

Run the project checks with:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

[MIT](LICENSE)
