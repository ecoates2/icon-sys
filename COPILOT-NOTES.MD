Good framing — since this becomes your template, it's worth covering the whole surface. Here's an overarching map of what a "complete" Rust CI looks like, organized by what you already have vs. what to add, with Rust-native tooling.

## What you already have (solid baseline)
- `rustfmt --check`, `clippy -D warnings`
- Build + test on Linux & Windows
- Dependency/build caching, concurrency cancellation
- Tag-triggered publish with a version/tag guard

## High-value additions (do these next)
These catch real bugs and are especially worth it for a *library* template:

1. **SemVer safety — `cargo-semver-checks`.** Runs against your published version and fails the PR if you made an accidental breaking API change without a major bump. Arguably the single most valuable add for a crate you publish.
2. **Supply chain — `cargo-deny`.** One tool covering RustSec advisories, license allow-lists, duplicate/banned crates, and source restrictions. Replaces `cargo-audit` and does more.
3. **Coverage — `cargo-llvm-cov`.** Generates lcov; upload to Codecov/Coveralls or just enforce a threshold. This is the "code coverage" item from your workplace.
4. **Feature & MSRV matrix — `cargo-hack`.** `--feature-powerset` builds every combination of your features (you have `folder-settings`), and it drives an MSRV check against the `rust-version` you set in Cargo.toml. Catches "works on my machine with all features on" breakage.
5. **Doc lint.** `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` catches broken intra-doc links — cheap, high signal for a published API.
6. **`--locked` builds + committed Cargo.lock** to detect dependency drift in CI.

## Commit & release automation (your workplace "other things")
This is where conventional commits pay off — they're the input to automated releases:

- **Enforce conventional commits.** Either on PR titles (`amannn/action-semantic-pull-request`) if you squash-merge, or on commits (`cocogitto`/`cog`, which is Rust-native). Pick based on your merge strategy.
- **`release-plz`.** The modern Rust release tool: it reads conventional commits, opens a "Release PR" that bumps the version and updates `CHANGELOG.md`, and on merge tags + publishes to crates.io. It essentially *replaces* your manual "bump version → tag → publish" loop and makes the tag/version guard unnecessary because it owns both. For a multi-crate project this is a big win.
- **Changelog** comes for free via `release-plz` (it wraps `git-cliff`).

## Hygiene / polish
- **Least-privilege `permissions:`** — default the workflow to `contents: read`, grant `write` only where needed (publish/release jobs). Important for a template.
- **`timeout-minutes`** on jobs (self-hosted runners can hang).
- **Pin third-party actions by commit SHA** (you're using `@v2`/`@v4` tags — fine, but SHA-pinning is the hardened posture).
- **Dependabot/Renovate** for dep + action updates.
- **`typos`** (`crate-ci/typos`) for docs/code spell-checking.
- **Status badges** in the README.

## Making it an actual template
Rather than copy-pasting YAML into each crate, factor the shared jobs into **reusable workflows** (`on: workflow_call`) in a central .github repo (or a `org/.github` repo). Each crate then has a tiny ci.yml that calls `your-org/.github/.github/workflows/rust-ci.yml@v1` and passes inputs (feature flags, MSRV, OS matrix). One place to update; all crates inherit.

---

A reasonable priority order for the template: **cargo-deny + cargo-semver-checks + cargo-hack (features/MSRV) + coverage** first (correctness/safety), then **conventional-commit enforcement + release-plz** (automation), then polish.

Want me to implement a subset here in `icon-sys` as the reference? I'd suggest starting with `cargo-deny`, `cargo-semver-checks`, and the `cargo-hack` feature/MSRV matrix added to ci.yml, plus tightening `permissions:` and adding job timeouts — and I can wire up `release-plz` to replace the current manual release flow if you want the full automation.