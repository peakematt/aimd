# Release Process

`aimd` uses SemVer and release tags in the form `vX.Y.Z`. The workspace version is shared by `aimd-core` and the `aimd` CLI package during v0 so the CLI and core library stay in lockstep.

## Automated Validation

Pull requests and pushes to `main` run `.github/workflows/ci.yml`:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace --all-targets --all-features`
- `cargo check --workspace --all-targets --all-features`
- `git diff --check`

The Rust checks run on Linux, macOS, and Windows. Formatting and whitespace checks run on Linux.

## Automatic Version Tags

After a PR is merged to `main`, `.github/workflows/ci.yml` reruns the validation suite. If validation passes, the `Tag release version` job finds the merged PR associated with the pushed commit. PRs without a SemVer label skip release tagging. PRs that should release need exactly one SemVer label:

- `major`
- `minor`
- `patch`

The job computes the next version from the latest `vX.Y.Z` tag. If no release tag exists yet, it starts from `v0.0.0`, so a `minor` label produces `v0.1.0`.

The release version must already be checked into the merged PR. CI verifies that the computed version matches `Cargo.toml`, tags the merge commit, and dispatches the release workflow. CI does not commit release bumps directly to `main`, so protected branch rules can require all changes to go through pull requests.

PRs with more than one release label fail the release-tagging job after tests pass. Add exactly one release label before merging any PR that should produce a versioned release.

## Tag Packaging

Pushing a tag that matches `v*.*.*` runs `.github/workflows/release.yml`. The workflow validates the tag shape, reruns the release validation suite, and builds binary archives for:

- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

The musl target is intentionally attempted from v0. If linking or dependency friction blocks the first release, decide explicitly whether to fix the target or drop it before publishing.

Each archive includes the `aimd` binary, `README.md`, and `LICENSE`. The workflow also creates `checksums.txt` with SHA256 checksums for every archive.

The release workflow also publishes agent-skill archives:

- `aimd-agent-skill-vX.Y.Z.tar.gz`
- `aimd-agent-skill-vX.Y.Z.zip`

Each skill archive contains one top-level `aimd/` skill folder with `SKILL.md`, an install helper, and skill installation notes. This gives humans and agents a stable URL they can download, inspect, install, and test without copying skill text out of the repository.

Tag-triggered runs upload the release bundle as GitHub Actions artifacts and create or update the public GitHub Release with those assets. Manual `workflow_dispatch` runs can still package an existing tag; when `publish_github_release` is disabled manually, the workflow creates a draft release for review.

## Manual Release Checklist

1. Confirm the working tree is clean and the intended release changes are merged to `main`.
2. Run the validation suite locally:

   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-features
   cargo test --workspace --all-targets --all-features
   cargo check --workspace --all-targets --all-features
   git diff --check
   ```

3. Dry-run packaging and publishing before creating a tag:

   ```bash
   cargo publish --dry-run -p aimd-core
   cargo publish --dry-run -p aimd
   ```

   The `aimd` package depends on `aimd-core` by version for crates.io packaging. Before `aimd-core` exists on crates.io, a full `aimd` dry run may fail dependency resolution; the release order is core first, then CLI.

4. Confirm the PR has exactly one release label when it should publish a release: `major`, `minor`, or `patch`.
5. Merge the PR and let CI create the version tag after validation passes.
6. Review the generated GitHub Release assets and `checksums.txt`.
7. Publish crates to crates.io only after explicit approval:

   ```bash
   cargo publish -p aimd-core
   cargo publish -p aimd
   ```

## Deferred Distribution Work

- Add `cargo-dist` once the CLI is useful enough for reviewed prebuilt installer automation.
- Consider artifact attestations for build provenance.
- Start package-manager distribution with a custom Homebrew tap, `.deb` release assets, and Windows zip assets. Defer Homebrew core, apt repository hosting, winget, Scoop, Chocolatey, and Linux distro repositories until usage justifies the maintenance.
