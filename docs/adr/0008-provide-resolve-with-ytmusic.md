# 0008. Decide how `ytmusic` provides `resolve`

- Status: Proposed
- Date: 2026-11-07
- Deciders: the user

## Context

Vehicoule's plugin providers table gives `resolve` (a track to a playable stream) to `ytmusic` alone. `ytmusic` gives
`radio` today; its crate documentation said `resolve` is missing because the player endpoint answered `UNPLAYABLE`
(corrected in the pull request that adds this ADR). That explanation was too short: the answer depends on a field the
first call did not send. Checked on 2026-11-07 from the user's Mac in France, one call per step,
with the official web client (`WEB_REMIX`, version `1.20261104.00.00`) and the headers the site sends, without an
account, without any other client identity, without any anti-bot token, and without requesting anything from
`googlevideo.com`:

1. `GET https://music.youtube.com/`: `302` to `consent.youtube.com` at first, then `200`, 795,119 bytes. The page holds
   the client version and a visitor id. It does not hold the signature timestamp (`STS`). That number, `20760` that
   day, is in the player script the page references (`/s/player/a5c2aafe/player_es6.vflset/en_US/base.js`, 200, about
   2.66 MB).
2. `POST /youtubei/v1/player?prettyPrint=false` with the client and a video id only: `200`, `playabilityStatus`
   `UNPLAYABLE`, reason "Video unavailable", whose error screen says "The page needs to be reloaded."; no
   `streamingData`. No answer spoke of bot detection.
3. The same call with `playbackContext.contentPlaybackContext.signatureTimestamp` `20760`, `racyCheckOk` and
   `contentCheckOk`: `200`, `OK`, with `streamingData` (`expiresInSeconds`, 1 `formats`, 16 `adaptiveFormats`,
   `serverAbrStreamingUrl`). The audio formats carry `signatureCipher` and no `url`; the URL inside it has an `n=`
   parameter and no `pot=`.
4. Nine video ids, one call each: eight `OK`, and in all eight every format (1 muxed, 16 to 18 adaptive) is ciphered,
   none has a plain `url`; one (`jNQXAC9IVRw`) is `UNPLAYABLE` ("Video unavailable") even with the timestamp. The ids
   were mostly major-label clips: no small-artist upload was checked.

Not checked: what `googlevideo.com` answers for a stream URL, and how to use `serverAbrStreamingUrl`.

So the official web client does answer, and its stream URLs cannot be used as given. Using them needs three things
the plugin does not have:

- the signature timestamp, which means fetching the page and then a script of 2.66 MB, more than the 1 MiB response
  body the conformance test allows (the app's real budget is not set);
- the signature (`s`) put back in order by functions defined in that script;
- the `n` parameter transformed by another function of the same script.

Both functions are JavaScript defined in the player script, which has its own version in its path. Their contents
were not read for this ADR, so how much of them a program could recognise without running them is not known.
[ADR 0001](0001-write-plugins-in-rust-as-wasm-components.md) and AGENTS.md rule 5 allow no JavaScript.

The user says that other client identities (the ones used before, Android VR and visionOS) are given plain URLs; that
was not checked here. Using one means telling the service the plugin is a client it is not, to get streams it does not
give the web client without the player's script.

## Options

1. **Run the player's JavaScript from the plugin**, with a JavaScript engine written in Rust inside the component.
   It is Rust, and it works for ciphered streams. It also adds a large dependency that runs code from the network, two
   fetches of about 3.5 MB per call, a script that changes with each player version, and it is the opposite of what the user
   wants to show (no JavaScript). The 2020 notice against youtube-dl cited in
   [ADR 0007](0007-keep-the-build-portable-to-other-forges.md) targeted a tool that does this deciphering; whether a
   plugin that does it is exposed the same way is a legal question this ADR does not answer.
2. **Another client identity.** Plain URLs, no JavaScript. The agents working on this repository do not write it: it is
   an identity the plugin does not have, used to obtain what the service does not give its web client. If the user
   wants this route, the user writes it and decides what it exposes the project to; this ADR is then superseded.
3. **`ytmusic` does not provide `resolve` until a source of full streams is chosen.** Vehicoule's providers table
   changes by an ADR there (`resolve` has no provider, so the core has none to ask and says so), and `ytmusic` stays the
   radio plugin it is. Nothing to write here but the corrected crate documentation.
4. **Take full streams from another service**, as another plugin. Nothing is known yet about which: deezer gives
   previews only. Each candidate needs its own verification (AGENTS.md rule 3) and the user's choice of what it
   provides.

## Decision

Proposed: option 3 now, with option 4 as the next piece of research, and the crate documentation corrected in the same
pull request. Options 1 and 2 stay the user's: this ADR gives the facts for them, and no agent writes either.

## Consequences

- If option 3: the crate documentation says what was found above and not "`UNPLAYABLE`" alone (done in the pull
  request that adds this ADR); Vehicoule's providers table and plugin host spec change in a pull request there; the
  core's routing has no `resolve` provider.
- If option 1 or 2: this ADR is superseded by one that says how, and `ytmusic` gains `resolve` with its own
  verification, tests, fuzz targets and proofs.
