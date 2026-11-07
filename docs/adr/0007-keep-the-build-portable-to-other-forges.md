# 0007. Keep the build and the checks portable to GitLab and Codeberg

- Status: Accepted
- Date: 2026-11-07
- Deciders: the user

## Context

- This repository holds the plugins exposed to takedown requests (`ytmusic`). The user said on 2026-11-06 that it will
  later be mirrored on GitLab and Codeberg under the same user and name; the accepted spec draws the consequence: "its
  build and its copy of `wit/` must not depend on GitHub-only features" (Vehicoule's
  [plugin host spec](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-host-spec.md), section 2).
- youtube-dl's GitHub repository was taken down after the RIAA's DMCA notice of 2020-10-23 (GitHub's `github/dmca`
  repository, `2020/10/2020-10-23-RIAA.md`) and reinstated on 2020-11-16 (GitHub's blog, "Standing up for developers:
  youtube-dl is back"). That post says that, for a repository taken down, GitHub "will ensure that repository owners
  can export their issues and PRs and other repository data […] where legally possible": what lives only on the forge
  is at its mercy, while every clone keeps the Git history. Both read on 2026-11-07.
- GitLab runs CI from `.gitlab-ci.yml`; Codeberg offers Forgejo Actions and Woodpecker CI. Not checked further: no
  mirror exists yet, and which CI each one uses is decided when it is created.
- This repository's checks need: a Linux machine, the pinned Rust toolchain, tools installed with `cargo install` at
  pinned versions, crates.io, and Vehicoule's Git repository (for `scripts/sync-contract.sh` and the conformance test,
  [ADR 0002](0002-pin-the-contract-and-check-plugins-in-the-real-host.md)).

## Options

1. **Every check a plain command; GitHub's workflow only installs tools and runs the commands; files for the other
   forges added when the mirrors exist.** Nothing to maintain for forges that do not exist yet; the commands of
   AGENTS.md are the contract every forge's CI follows.
2. **Write `.gitlab-ci.yml` and a Forgejo workflow now.** Ready on day one, but untested files that nothing runs, and
   that drift from the GitHub workflow until a mirror exists.
3. **No rule.** The GitHub workflow grows actions and settings that the mirrors cannot reproduce.

## Decision

Option 1, recommended:

- Every check that decides pass or fail is a command listed in AGENTS.md: a tool at a pinned version, or a script in
  `scripts/` (`check-layout.sh`, `sync-contract.sh`, `build-plugins.sh`, `prove.sh`, `fuzz.sh`). The GitHub workflow
  installs the tools and runs those commands, and nothing else decides a result there.
- GitHub-only conveniences are allowed where they decide nothing: `actions/checkout`, `Swatinem/rust-cache`,
  `CODEOWNERS`, the pull request template, private vulnerability reporting, and `zizmor`, which audits the GitHub
  workflow files themselves. `cargo-deny` is installed with `cargo install`, not through its GitHub action.
- Vehicoule is fetched over plain Git. `scripts/sync-contract.sh` takes `VEHICOULE_REPO` to fetch it from a mirror; the
  conformance test's Cargo dependency keeps Vehicoule's GitHub URL, which a mirror can still reach, until Vehicoule is
  mirrored too.
- What matters lives in the repository: decisions in ADRs, findings about the service in the plugin's crate
  documentation ([`docs/plugin-crates.md`](../plugin-crates.md), section 6), never only in an issue or a pull request
  thread.
- When a mirror is created, its CI file is added in a pull request of its own and runs the same commands.

## Consequences

- A step that needs a GitHub action to decide a result is refused in review.
- A takedown on GitHub stops its CI and hides its issues and pull requests; the history, the rules and the checks
  survive in every clone and mirror.
- Mirroring itself (pushing to GitLab and Codeberg, keeping them in sync) is not set up by this ADR.
