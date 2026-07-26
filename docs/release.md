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

After a PR is merged to `main`, `.github/workflows/ci.yml` reruns the validation suite. If validation passes, the `Tag release version` job finds the merged PR associated with the pushed commit and requires exactly one SemVer label:

- `major`
- `minor`
- `patch`

The job computes the next version from the latest `vX.Y.Z` tag. If no release tag exists yet, it starts from `v0.0.0`, so a `minor` label produces `v0.1.0`.

When the computed version differs from the workspace version in `Cargo.toml`, CI updates the workspace version, refreshes `Cargo.lock`, commits `chore: release vX.Y.Z` to `main`, and tags that release commit. If the workspace already has the computed version, CI tags the merge commit directly.

PRs without exactly one release label fail the release-tagging job after tests pass. Add the release label before merging so version tags are deterministic.

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

Tag-triggered runs upload the release bundle as GitHub Actions artifacts. Creating or updating the GitHub Release is a separate manual `workflow_dispatch` path with `publish_github_release` set to `true`; the job targets the `github-release` environment and creates a draft release.

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

4. Confirm the PR has exactly one release label: `major`, `minor`, or `patch`.
5. Merge the PR and let CI create the version tag after validation passes.
6. Review the release workflow artifacts and `checksums.txt`.
7. If approved, run the release workflow manually for the same tag with `publish_github_release` enabled. Review and publish the generated draft GitHub Release.
8. Publish crates to crates.io only after explicit approval:

   ```bash
   cargo publish -p aimd-core
   cargo publish -p aimd
   ```

## Deferred Distribution Work

- Add `cargo-dist` once the CLI is useful enough for reviewed prebuilt installer automation.
- Consider artifact attestations for build provenance.
- Start package-manager distribution with a custom Homebrew tap, `.deb` release assets, and Windows zip assets. Defer Homebrew core, apt repository hosting, winget, Scoop, Chocolatey, and Linux distro repositories until usage justifies the maintenance.
