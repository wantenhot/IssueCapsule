# IssueCapsule

Turn "works on my machine" into a reproducible bug.

```bash
issuecap create \
  https://github.com/foo/bar/issues/123 \
  --run "python reproduce.py" \
  --expect "IndexError"
```

```text
BUG REPRODUCED ✓

Saved:
issue-123.icap
```

Then reproduce it again:

```bash
issuecap run issue-123.icap
```

IssueCapsule does not fix bugs.

It makes bugs reproducible.

> [!WARNING]
> A capsule contains a command that runs in Docker. Review `.icap` files from
> untrusted sources before running them.

## Requirements

- Git
- Docker with a running Linux container daemon
- Network access to public GitHub repositories and Python package indexes

IssueCapsule itself runs on Windows, Linux, and macOS. Reproduction always runs
inside a Linux Docker container.

## Install from source

Install the Rust toolchain, then run:

```bash
cargo install --path .
```

Confirm that the required tools and network access are available:

```bash
issuecap doctor
```

## Create a capsule

Pass a public GitHub issue URL, a reproduction command, and the text that must
appear in standard output or standard error:

```bash
issuecap create \
  https://github.com/foo/bar/issues/123 \
  --run "python reproduce.py" \
  --expect "IndexError"
```

IssueCapsule fetches the issue metadata, clones the repository at its default
branch HEAD, detects its Python dependency file, builds an isolated Docker
image, and runs the command. It saves `issue-123.icap` only when the expected
text appears.

Dependency installation follows this fixed order:

1. `requirements.txt` → `pip install -r requirements.txt`
2. `pyproject.toml` → `pip install .`
3. Neither file → no dependency installation

The Docker image is `python:3.12-slim` in v0.1.

## Run or verify a capsule

Run the pinned repository commit and print its output:

```bash
issuecap run issue-123.icap
```

Run it and check the recorded expected error:

```bash
issuecap verify issue-123.icap
```

`verify` exits with code `0` and prints `BUG REPRODUCED ✓` when the expected
text appears. Otherwise, it exits with code `1` and prints
`BUG NOT REPRODUCED ✗`.

## Capsule format

An `.icap` file is editable TOML, not a compressed or binary format:

```toml
version = 1

[issue]
url = "https://github.com/foo/bar/issues/123"
number = 123
title = "Crash when input is empty"

[source]
repository = "https://github.com/foo/bar.git"
commit = "0123456789abcdef0123456789abcdef01234567"

[environment]
language = "python"
docker_image = "python:3.12-slim"

[install]
strategy = "requirements"
command = "pip install -r requirements.txt"

[reproduce]
command = "python reproduce.py"
expected_error = "IndexError"
```

## Current limitations

- GitHub only
- Python only
- Docker required
- Reproduction command must currently be provided manually
- Public repositories only
- `requirements.txt` and `pyproject.toml` are the only detected dependency files

IssueCapsule v0.1 does not analyze issue prose, infer reproduction steps, fix
bugs, use AI, or upload source code and results to a service.

## Develop

Run the local quality checks:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Docker integration is intentionally outside the default unit test suite.

## License

[MIT](LICENSE)
