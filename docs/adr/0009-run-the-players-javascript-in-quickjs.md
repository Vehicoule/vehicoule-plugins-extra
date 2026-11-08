# 0009. Run the player's JavaScript in QuickJS so that `ytmusic` can resolve

- Status: Accepted
- Date: 2026-11-08
- Deciders: the user

## Context

[ADR 0008](0008-provide-resolve-with-ytmusic.md) found that the official web client gets stream URLs that need the
player script's own functions (the signature and the `n` parameter), and left open how. On 2026-11-07 the user said
`ytmusic` is their `resolve` source. On 2026-11-08 the user asked to try both engines that could run that script and to
take the most pragmatic one. This ADR records the tries and what accepting the result changes.

All measurements below were made in a Linux container (x86_64, Intel Xeon at 2.30 GHz, 4 cores), on 2026-11-08, with
`rquickjs` 0.14.0 (QuickJS, C), `boa_engine` 0.22.0 (Rust), Wasmtime 48.0.8 (the host's LTS line, Cranelift), Rust
1.99.1 and clang 18.1.3. Each figure is one run; `solve` was repeated three times in the same instance. Nothing was
run on the Xiaomi 14 or the Mac, and no plugin was built: the probes are scratch crates, not committed.

**The script.** The player of that day is `/s/player/a5c2aafe/player_es6.vflset/en_US/base.js`, 2,658,728 bytes, with
`signatureTimestamp` 20760 in it. Parsed with `oxc_parser` 0.157 (native, release): 47 to 51 ms, no error, 6,992
statements in its main function. The filter yt-dlp-ejs 0.8.0 applies before running it keeps 6,845 of them
(2,637,830 bytes): running the script means running almost all of it. It uses 67 kinds of syntax node (1,346 classes,
233 `await`, 34 `yield`, 251 regular expressions, 2,370 optional chains) and 105 global names (`Math`, `Map`, `Set`,
`Promise`, `Uint8Array`, `DataView`, `BigInt`, `Date`, `Intl`, `Symbol`, about forty browser APIs). The code that
deciphers is reached through a string table (`w[2]`) and numeric dispatchers, so it cannot be found without running
the script. A hand-written interpreter would be a complete JavaScript engine.

**The call.** The script runs after a few stand-ins (an empty `document` and `navigator`, a `location` object,
`window` as the global) with its `var window=this;` removed, as yt-dlp-ejs does. It finds the one function whose body
calls `.set("alr","yes")` (`Bs` that day), builds a URL object with it, sets `n`, and calls the first method of that
object's prototype that is not `constructor`, `set`, `get` or `clone`; the object then holds the deciphered `s` and
the new `n`. The call was written for this test after reading yt-dlp-ejs's solver; its license was not checked.

**The engines.**

| | QuickJS | boa |
|---|---|---|
| Native, load of the script | 232 ms | 4.97 s |
| Native, peak memory | 37.6 MiB | 309 MiB |
| Native, one `solve` | 0.6 to 0.8 ms | 4.2 to 4.5 ms |
| Wasm module (opt-level 3, no LTO), imports | 1,219,815 bytes, 0 | 7,961,712 bytes, 0 |
| Wasm, Cranelift compile | 1.48 s | 7.04 s |
| Wasm, load: time, fuel, linear memory | 347 ms, 3.15 G, 32.8 MiB | 9.75 s, 48.3 G, 207 MiB |
| Wasm, one `solve`: time, fuel | 1.0 to 1.2 ms, 11.6 M | 9.6 to 10.1 ms, 77 M |

Earlier size probes (a script of one line, opt-level `s` or `z`, LTO, stripped): QuickJS 815 KB, boa 3.20 MB. The four
runs (both engines, native and Wasm) gave the same deciphered `s` and `n` for the same inputs. Times include fuel
metering. A fresh store and instance per call (host spec, section 3) means every call pays the load.

**What each engine needs to run with no import.** QuickJS: an exported `__rquickjs_host_now_us` returning `f64`
(`#[no_mangle]`, which `unsafe_code = "deny"` rejects) and `set_max_stack_size(0)`, because its stack check by address
fails at once in Wasm. boa: a fixed clock, and a randomness backend, because the script calls `Math.random()` at load
and boa's call panics without one (a plugin has none, AGENTS.md rule 4; the probe used a fixed stream). QuickJS's
`Math.random` ran with no import.

**Does the service accept the result?** On 2026-11-08, from the container, one run, no account, no token: `/player`
(web client `WEB_REMIX` 1.20261104.00.00, `signatureTimestamp` 20760) answered `200`, `OK`, 78,298 bytes, 1 muxed and 18
adaptive formats, all with `signatureCipher`. For the Opus audio format (itag 251), QuickJS deciphered `s` and
transformed `n`, and the URL was requested from `googlevideo.com` with the `Origin`, `Referer` and `User-Agent` of the
web site: `302` to another `googlevideo.com` host, then `206`, `audio/webm`, `Content-Range: bytes 0-1/3433755` (the
`contentLength` of the answer). Ranges 0 to 1,048,575, 2,000,000 to 2,099,999 and 3,400,000 to the end answered `206`
with the right sizes, and the first bytes are the WebM header (`1a45dfa3`). With the `n` left as the service gave it
and the same deciphered `s`: `403`. The URL had no `pot=` parameter. The service places the container's address in no
country (the crate's documentation records empty search results from it); `/player` and the stream worked from it all
the same.

Not checked: other tracks, other formats, small-artist uploads, playing a whole track or several in a row, the
service's limits, whether the headers sent are all needed, any address in France, what the plugin's requests (not the
container's) look like to the service, and the terms of use. A change of the player (new version, new shape) can break
the way the function is found; nothing here says how often that happens.

## Options

1. **QuickJS, inside the `ytmusic` plugin.** `rquickjs` 0.14.0 with its C sources, built for `wasm32-unknown-unknown`
   with clang. 1.2 MB Wasm, 347 ms and 33 MiB per call in the probe. Costs: a C dependency that runs code from the
   network, and the exceptions listed under Decision.
2. **boa, inside the plugin.** Rust, so no C dependency; it still runs JavaScript (the same exception to rule 5) and
   needs a `#[no_mangle]` randomness hook of its own. 8 MB (3.2 MB size-optimized), 9.75 s and 207 MiB per call in the
   probe, 7 s to compile at each launch. Too slow and too large for a call a listener waits for.
3. **The engine in the host, offered to plugins by the contract.** The plugin stays pure Wasm and small; Vehicoule adds
   an engine dependency and a contract change, which starts in Vehicoule (rule 6). Nothing measured.
4. **A hand-written interpreter.** See the script above: it is a JavaScript engine to write, test and prove.

[ADR 0008](0008-provide-resolve-with-ytmusic.md)'s other options stay as they were: a different client identity is not
written by the agents, and `resolve` without a provider or from another service were not taken by the user.

## Decision

Option 1, accepted by the user on 2026-11-08 ("on est bon", answering this ADR's list of exceptions). It is the one
that worked within a budget a person can wait for; boa did not. The exceptions it rests on:

- **Rule 5**, "Rust only, no JavaScript" ([ADR 0001](0001-write-plugins-in-rust-as-wasm-components.md), which copies
  Vehicoule's decision), for one engine that runs the service's own script and a few lines of glue of ours, written as
  JavaScript text: the stand-ins for the browser's globals and the function that calls the script's own (the earlier
  wording, "never code of ours", was wrong about the glue; corrected on 2026-11-08, see the addendum). AGENTS.md says
  so in the same pull request as this ADR. Vehicoule's ADR 0001 covers its app and its crates ("No QuickJS in the app
  binary"); the engine lives inside a plugin's Wasm, so no ADR is needed there unless the user wants one.
- **Rule 11**: `rquickjs` and QuickJS's C sources handle untrusted content (a script from the network). They run
  inside the Wasm sandbox, with no import, under fuel and memory limits.
- **Rule 9 and [ADR 0005](0005-allow-unsafe-only-in-the-generated-bindings.md)**: one `#[no_mangle]` function, which
  needs `unsafe`. A new ADR, or an addition to 0005, names the exact module and function.
- **CI and other forges**: building QuickJS needs clang able to target `wasm32-unknown-unknown`, pinned like the other
  tools and installed on GitLab and Codeberg too ([ADR 0007](0007-keep-the-build-portable-to-other-forges.md)).
  Only clang 18.1.3 in the container is known to build it; CI was not tried.

## Consequences

- `ytmusic` declares `resolve` and, with it, hosts for the streams: `*.googlevideo.com` is a suffix wildcard the
  manifest can state (host spec, section 4), and the player edge checks every redirect hop against it (section 5).
  Whether the platform players send the headers the service accepted is not checked.
- Each `resolve` fetches the music page (to learn the player's path) and the player (2.66 MB), because the plugin
  keeps nothing. The page was 293,747 bytes from the container and 795,119 from the Mac (ADR 0008). The host's maximum
  response size has no value yet and must be above 2.7 MB for this plugin; the conformance test's 1 MiB is below it.
  Whether the edge caches these responses is Vehicoule's decision and is not taken here.
- The budgets the host needs are above the probe's: fuel above 3.2 G, linear memory above 33 MiB plus the copies of
  the script, a wall time above the load. They are set from measurements on the Xiaomi 14 and the Mac, by the user, in
  an ADR (host spec, section 9); the container's figures are a lower bound, not a budget.
- Tests cannot reach the network (rule 7). The real player is not in the repository: a test uses a script of our own
  with the same anchor shape, and checking against the real player is a procedure written down and run by hand. Whether
  the real player may be kept as a fixture is the user's call; its license was not checked.
- What the plugin reads from outside gains fuzz targets (rule 15): the page, the player's path, the cipher, the
  function's result.
- `ytmusic` grows from about 192 KB (core module) by the engine's 1.2 MB before its own code, and compiles in about
  1.5 s at each launch in Wasmtime on the container.
- The legal question ADR 0008 raised for option 1 (the 2020 notice against youtube-dl) is not answered here and is the
  user's.
- When this ADR is accepted, ADR 0008 becomes "Superseded by 0009".

## Addendum, 2026-11-08, after acceptance: other tracks, the visitor id, the clients

Written after the user accepted this ADR, from further runs the same day. It adds measurements and one correction; it
changes none of the options or of the decision.

**Other tracks, without the visitor id.** The same container, the plugin built and run in the plugin host (Vehicoule's
pull request 50) against the real service, one run per id, no account, no token. `resolve` was given the video id
directly, because search returns nothing from the container (ADR 0008). Of 14 ids of well-known songs, one gave a stream
(`dQw4w9WgXcQ`: `206`, `audio/webm`). Seven were answered by `/player` with `LOGIN_REQUIRED`, "Sign in to confirm you're
not a bot" (`9bZkp7q19f0`, `JGwWNGJdvx8`, `fJ9rUzIMcZQ`, `RgKAFK5djSk`, `YQHsXMglC9A`, `e-ORhEE9VVg`, `2Vv-BfVoq4g`),
and six with `UNPLAYABLE`, "Video unavailable" (`kJQP7kiw5Fk`, `hTWKbfoikeg`, `OPf0YbXqDm0`, `4D7u5KF7SP8`,
`CevxZvSJLk8`, `09R8_2nJtjg`). The `visionOS` and `ANDROID_VR` clients, with the values of yt-dlp's client table of the
same day, gave the same answers for the same ids.

**The visitor id.** The bot check goes away when the `/player` call carries the visitor id the music page gives
(`"VISITOR_DATA"` in its configuration), in the `X-Goog-Visitor-Id` header and in the client's `visitorData`, as the web
site sends it; cookies alone changed nothing. With it, the seven `LOGIN_REQUIRED` ids answer `OK` on the web client and
on `visionOS`. The six `UNPLAYABLE` ids stay so on every client: the service does not serve them to this address, for a
reason it does not give. The plugin reads the id from the page it already fetches and sends it with `/player` (not with
search and radio, which were not changed).

**The streams, with the visitor id.** Of the eight ids that answer `OK`, one plays (`dQw4w9WgXcQ`) and seven get `403`
from `googlevideo.com`, each one every time. The same `403` comes with every other way of asking that was tried: the
plugin's own URL (the signature and `n` put in order by QuickJS); the `visionOS` client's URL, which holds no signature,
no `n` and no `pot`, so there is nothing for the plugin to put together; the `ANDROID_VR` client's; the first segment of
the HLS audio playlist that `visionOS` gives (the manifest and the playlist answer `200`); with and without the web
site's `Origin`, `Referer` and `User-Agent`; with a `Range` header, a `range` parameter and none; over HTTP/1.1. So the
`403` is not the signature, the `n`, the URL's assembly or the headers.

What this says, and does not say. The stream URLs carry `ip=fda3:…`, a private address, so the service sees the
container's traffic as unlike a person's; and yt-dlp's client table of the same day says the web client needs a token
(`pot`) for HTTPS and DASH streams, and notes "intermittent/selective" token enforcement for `ANDROID_VR`. Either the
container's address or a token the service asks for most videos could be the cause; the two cannot be told apart from
here, and the plugin does not make a token (the project does not forge one). A run from a home connection (the Mac, or
the Xiaomi 14 on a home network) is the test that separates them. Until then, nothing here says how many tracks
`resolve` gives in use.

**What `visionOS` would change, if the user wanted it.** Its `/player` answer, from `www.youtube.com`, has direct URLs
for every format (Opus `251`, AAC `140`) with no signature and no `n`: no player script to fetch (2.66 MB), no engine,
no `rquickjs`, no C dependency, and none of this ADR's exceptions. Costs: the plugin would present itself as another
client, which ADR 0008 left to the user; yt-dlp's table says `ANDROID_VR` has had every format refused since 2026-08-17,
so the same can happen to `visionOS`; and "made for kids" videos are not served to it. Not decided here.

**The stream headers.** The stream of `dQw4w9WgXcQ` was requested with and without the `Origin`, `Referer` and
`User-Agent` of the web site: `302` then `206`, `audio/webm`, `Content-Range: bytes 0-1023/3433755` both times. For that
stream the headers were not needed; for other streams and other addresses, not checked.

**Correction.** The Decision's first bullet, and AGENTS.md rule 5, said the engine runs "never code of ours". The plugin
also runs 866 bytes of JavaScript of its own (counted in the module that holds them, not merged yet), as constant text:
the stand-ins for the browser's globals and the function that calls the script's own. Its method is yt-dlp-ejs's,
written again; that project's license was not checked. The Decision's first bullet and rule 5 now say so.
