# ci/

Staging area for GitHub Actions workflow changes.

This directory exists because session credentials cannot write
`.github/workflows/*` — see admin `DECISIONS.md` ADR-8. To change CI:

1. Put the intended workflow file in `workflows/`.
2. A maintainer promotes it with the admin `rollout/apply-ci-folders.sh`
   script.

## Promoted, 2026-09-22

Both files that were staged here are now live, moved by the rollout
script rather than edited: `workflows/docs.yml` is
`.github/workflows/docs.yml` and `workflows/rust.yml` is
`.github/workflows/rust.yml`. Nothing is pending. Read the workflows
themselves rather than a description of them here.

## What still lives here

- **`rust/run.sh`** is the Rust gate itself: formatting, build, the
  shared fixtures, the in-language suites, doctests, clippy, and a
  lockfile check that exempts only the sibling crates' versions, on the
  MSRV pinned in `rs/Cargo.toml`. `.github/workflows/rust.yml` clones
  `tabnas/parser`, `tabnas/json`, `tabnas/jsonic`, `tabnas/directive`,
  `tabnas/support`, `tabnas/path` and `tabnas/debug` beside the checkout,
  because the crate takes all seven as path dependencies, and then calls
  it; you can run it too. `make test-rs` is the fast local loop.
