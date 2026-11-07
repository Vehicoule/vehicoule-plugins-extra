# vehicoule-plugins-extra

Content plugins for [Vehicoule](https://github.com/Vehicoule/Vehicoule) that may be the target of takedown requests:
WebAssembly components, written in Rust, that the app runs in a sandbox. Its plugin is `ytmusic`, which gives radio
candidates from YouTube Music; resolving a track to a playable stream is planned. The other plugins live in
[`vehicoule-plugins`](https://github.com/Vehicoule/vehicoule-plugins).

This repository is meant to be mirrored on GitLab and Codeberg, so its build and checks run anywhere
([ADR 0007](docs/adr/0007-keep-the-build-portable-to-other-forges.md)).

Licensed under the [GNU General Public License v3.0](LICENSE) ([ADR 0004](docs/adr/0004-license-the-plugins-under-gpl-3.0.md)).

Start with [`AGENTS.md`](AGENTS.md): the rules every contributor, human or agent, follows. How a plugin is written is
in [`docs/plugin-crates.md`](docs/plugin-crates.md), decisions in [`docs/adr/`](docs/adr/), sources in
[`docs/references.md`](docs/references.md).
