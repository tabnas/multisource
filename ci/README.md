# ci/

The scripts the CI workflows run, kept here so that you can run the same
gate locally: `rust/run.sh` is the Rust gate, and
`.github/workflows/rust.yml` runs it (see "What still lives here").

To change CI, edit `.github/workflows/` in a reviewed pull request.
Session credentials push workflow files (admin `DECISIONS.md` ADR-8, as
amended 2026-09-24), so staging a workflow here first for a maintainer
to promote is optional. Two cases also involve admin:

- A workflow admin keeps a template for
  (`rollout/workflows/multisource__<file>`) is mirrored in that
  template at the same time, or admin `scripts/verify.sh` reports the
  drift and a maintainer's next `rollout/apply-workflows.sh --apply`
  would push the old text back.
- The stamped `clib.yml` and `clib-release.yml` (each carries a
  `tabnas-clib-template` marker) are never edited by hand. Change admin
  `tasks/clib-template/` and re-stamp with `tasks/adopt-clib.sh`, which
  writes both workflows straight into `.github/workflows/`. The new
  stamp lands in this repository's own reviewed pull request.

Sessions still cannot push tags, so a maintainer pushes any tag that a
tag-triggered workflow needs.

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
