//! Conformance of this repository's plugins to Vehicoule's plugin host (ADR 0002).
//!
//! Every component in the directory named by `VEHICOULE_PLUGINS`, which `scripts/build-plugins.sh` fills, is installed
//! in the real host: its size, its compilation, its imports, `describe` with every request refused, the manifest rules
//! and the capabilities matched against the exports (Vehicoule, `docs/plugin-host-spec.md`, sections 3 to 5). A plugin
//! the app would refuse fails here first. A missing or empty directory fails the test: it is never skipped.
//!
//! The capabilities a plugin provides are called through the host as the app calls them, on a network that serves
//! made-up answers in the service's shape and records every request (section 9): the component the app would run,
//! not the crate the unit tests call. `ytmusic`'s `radio` is the first.
//!
//! The budgets below are arbitrary and generous, like the host's own tests: the spec gives no value yet, and the app's
//! real budgets will come from measurements (section 9). Passing here does not show that a plugin fits them.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    use vehicoule_plugin_host::{
        ArtistRef, ByteSize, Capability, Fuel, HostConfig, HttpBudget, HttpRequest, HttpResponse,
        LogBudget, Method, Network, NetworkError, PluginError, PluginFailure, PluginHost,
        RequestLimits, Ticks, Track,
    };

    /// A network that never opens a socket and records every request that reaches it. During install the host refuses
    /// every request before the network (section 3), so nothing may ever be recorded here.
    #[derive(Default)]
    struct NoNetwork {
        requests: Mutex<Vec<HttpRequest>>,
    }

    impl Network for NoNetwork {
        fn fetch(
            &self,
            request: &HttpRequest,
            _limits: RequestLimits,
        ) -> Result<HttpResponse, NetworkError> {
            self.requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(request.clone());
            Err(NetworkError::Transport(String::from(
                "no network in the conformance test",
            )))
        }
    }

    fn config() -> HostConfig {
        HostConfig::new(
            ByteSize::new(8 << 20),
            Fuel::new(50_000_000),
            // Never reached: nothing ticks this host.
            Ticks::new(10),
            ByteSize::new(32 << 20),
            ByteSize::new(64 << 10),
            HttpBudget::new(3, ByteSize::new(1 << 20), Duration::from_secs(10)),
            LogBudget::new(20, ByteSize::new(512)),
        )
    }

    /// Every `.wasm` file of the directory named by `VEHICOULE_PLUGINS`, sorted.
    fn components() -> Vec<PathBuf> {
        let dir = std::env::var_os("VEHICOULE_PLUGINS")
            .expect("VEHICOULE_PLUGINS must name the output of scripts/build-plugins.sh");
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", Path::new(&dir).display()))
            .map(|entry| entry.expect("a directory entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "wasm"))
            .collect();
        paths.sort();
        paths
    }

    #[test]
    fn every_plugin_installs_under_its_own_id() {
        let paths = components();
        // At least the template: an empty directory means the build did not run, not that all is well.
        assert!(
            paths
                .iter()
                .any(|p| p.file_stem().is_some_and(|s| s == "template")),
            "no template.wasm among {paths:?}"
        );

        let network = Arc::new(NoNetwork::default());
        let host = PluginHost::new(config(), Arc::clone(&network) as Arc<dyn Network>)
            .expect("the host starts");
        for path in paths {
            let bytes = std::fs::read(&path).expect("the component is readable");
            let plugin = host
                .install(&bytes)
                .unwrap_or_else(|e| panic!("{} does not install: {e}", path.display()));
            let stem = path.file_stem().and_then(|s| s.to_str());
            assert_eq!(
                Some(plugin.manifest().id().as_str()),
                stem,
                "{}: the manifest id must be the file name, which is the crate's directory",
                path.display()
            );
        }
        assert_eq!(
            network
                .requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .len(),
            0,
            "a request reached the network during install"
        );
    }

    type Answer = dyn Fn(&HttpRequest) -> Result<HttpResponse, NetworkError> + Send + Sync;

    /// A network that never opens a socket: it records every request and answers each with `answer`.
    struct ScriptedNetwork {
        requests: Mutex<Vec<HttpRequest>>,
        answer: Box<Answer>,
    }

    impl ScriptedNetwork {
        fn new(
            answer: impl Fn(&HttpRequest) -> Result<HttpResponse, NetworkError> + Send + Sync + 'static,
        ) -> Arc<Self> {
            Arc::new(Self {
                requests: Mutex::new(Vec::new()),
                answer: Box::new(answer),
            })
        }

        fn requests(&self) -> Vec<HttpRequest> {
            self.requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl Network for ScriptedNetwork {
        fn fetch(
            &self,
            request: &HttpRequest,
            _limits: RequestLimits,
        ) -> Result<HttpResponse, NetworkError> {
            self.requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(request.clone());
            (self.answer)(request)
        }
    }

    // Made-up answers of `YouTube Music` in the shape `plugins/ytmusic`'s crate documentation ("Service") says was
    // checked on 2026-11-07. Nothing here was recorded from the service.

    const LINK_ARTIST: &str = r#"{"text":"@NAME@","navigationEndpoint":{"browseEndpoint":{"browseId":"@ID@","browseEndpointContextSupportedConfigs":{"browseEndpointContextMusicConfig":{"pageType":"MUSIC_PAGE_TYPE_ARTIST"}}}}}"#;
    const LINK_ALBUM: &str = r#"{"text":"@NAME@","navigationEndpoint":{"browseEndpoint":{"browseId":"@ID@","browseEndpointContextSupportedConfigs":{"browseEndpointContextMusicConfig":{"pageType":"MUSIC_PAGE_TYPE_ALBUM"}}}}}"#;

    const SEARCH_ANSWER: &str = r#"{"contents":{"tabbedSearchResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[{"musicShelfRenderer":{"contents":[@ITEM@]}}]}}}}]}}}"#;
    const RADIO_ANSWER: &str = r#"{"contents":{"singleColumnMusicWatchNextResultsRenderer":{"tabbedRenderer":{"watchNextTabbedResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"musicQueueRenderer":{"content":{"playlistPanelRenderer":{"contents":[@ITEMS@]}}}}}}]}}}}}"#;

    fn link(template: &str, id: &str, name: &str) -> String {
        template.replace("@ID@", id).replace("@NAME@", name)
    }

    /// A search answer whose only song is `id`, titled `title`, by "Artist One", on "Album One", of 3:20.
    fn search_answer(id: &str, title: &str) -> String {
        let artist = link(LINK_ARTIST, "UCartist1", "Artist One");
        let album = link(LINK_ALBUM, "MPREalbum1", "Album One");
        let item = r#"{"musicResponsiveListItemRenderer":{"thumbnail":{"musicThumbnailRenderer":{"thumbnail":{"thumbnails":[{"url":"https://yt3.googleusercontent.com/madeup=w60-h60","width":60,"height":60}]}}},"flexColumns":[{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"@TITLE@"}]}}},{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[@ARTIST@,{"text":" • "},@ALBUM@,{"text":" • "},{"text":"3:20"}]}}}],"playlistItemData":{"videoId":"@ID@"}}}"#
            .replace("@TITLE@", title)
            .replace("@ARTIST@", &artist)
            .replace("@ALBUM@", &album)
            .replace("@ID@", id);
        SEARCH_ANSWER.replace("@ITEM@", &item)
    }

    /// A song of a radio answer: by "Artist Two", of 4:05, with one 544-pixel picture.
    fn radio_item(id: &str, title: &str) -> String {
        let artist = link(LINK_ARTIST, "UCartist2", "Artist Two");
        r#"{"playlistPanelVideoRenderer":{"videoId":"@ID@","title":{"runs":[{"text":"@TITLE@"}]},"longBylineText":{"runs":[@ARTIST@,{"text":" • "},{"text":"2013"}]},"lengthText":{"runs":[{"text":"4:05"}]},"thumbnail":{"thumbnails":[{"url":"https://yt3.googleusercontent.com/madeup=w544-h544","width":544,"height":544}]}}}"#
            .replace("@ID@", id)
            .replace("@TITLE@", title)
            .replace("@ARTIST@", &artist)
    }

    fn radio_answer(items: &[String]) -> String {
        RADIO_ANSWER.replace("@ITEMS@", &items.join(","))
    }

    fn json(body: String) -> HttpResponse {
        HttpResponse::new(
            200,
            vec![(
                String::from("content-type"),
                String::from("application/json; charset=UTF-8"),
            )],
            body.into_bytes(),
        )
    }

    /// The service as the plugin's two requests see it: a search finds "Song One" as `seedvid0001`, and the radio of that
    /// song holds the song itself, an excluded one and two others.
    fn youtube_music(request: &HttpRequest) -> Result<HttpResponse, NetworkError> {
        match request.url.as_str() {
            "https://music.youtube.com/youtubei/v1/search?prettyPrint=false" => {
                Ok(json(search_answer("seedvid0001", "Song One")))
            }
            "https://music.youtube.com/youtubei/v1/next?prettyPrint=false" => {
                Ok(json(radio_answer(&[
                    radio_item("seedvid0001", "Song One"),
                    radio_item("one00000001", "Song Two"),
                    radio_item("excluded001", "Song Excluded"),
                    radio_item("two00000002", "Song Three"),
                ])))
            }
            other => Err(NetworkError::Transport(format!("unexpected url {other}"))),
        }
    }

    /// The seed the core hands to `radio`: a track of another plugin, with no id of `YouTube Music`'s.
    fn seed() -> Track {
        let mut artist = ArtistRef::default();
        artist.id = String::from("other-artist");
        artist.name = String::from("Artist One");
        let mut seed = Track::default();
        seed.id = String::from("other-track");
        seed.title = String::from("Song One");
        seed.artists = vec![artist];
        seed.duration = Some(Duration::from_secs(200));
        seed
    }

    fn ytmusic_in(
        network: &Arc<ScriptedNetwork>,
    ) -> (PluginHost, vehicoule_plugin_host::InstalledPlugin) {
        let path = components()
            .into_iter()
            .find(|path| path.file_stem().is_some_and(|stem| stem == "ytmusic"))
            .expect("ytmusic.wasm is among the built plugins");
        let host = PluginHost::new(config(), Arc::clone(network) as Arc<dyn Network>)
            .expect("the host starts");
        let plugin = host
            .install(&std::fs::read(path).expect("the component is readable"))
            .expect("ytmusic installs");
        (host, plugin)
    }

    #[test]
    fn ytmusic_declares_radio_and_its_one_host() {
        let (_, plugin) = ytmusic_in(&ScriptedNetwork::new(youtube_music));
        let manifest = plugin.manifest();
        assert_eq!(
            manifest.capabilities().iter().copied().collect::<Vec<_>>(),
            [Capability::Radio]
        );
        assert!(manifest.allows_host("music.youtube.com"));
        assert!(!manifest.allows_host("www.youtube.com"));
        assert!(!manifest.allows_host("yt3.googleusercontent.com"));
    }

    #[test]
    fn ytmusic_gives_the_radio_of_a_seed_through_the_host() {
        let network = ScriptedNetwork::new(youtube_music);
        let (host, plugin) = ytmusic_in(&network);

        let call = host.radio(&plugin, &seed(), &[String::from("excluded001")], 5);

        let tracks = call.outcome.expect("the radio is answered");
        // The service's order, without the seed's own song, the excluded id, or a repeat.
        assert_eq!(
            tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["one00000001", "two00000002"]
        );
        assert_eq!(
            tracks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
            ["Song Two", "Song Three"]
        );
        let first = &tracks[0];
        assert_eq!(
            first
                .artists
                .iter()
                .map(|a| (a.id.as_str(), a.name.as_str()))
                .collect::<Vec<_>>(),
            [("UCartist2", "Artist Two")]
        );
        assert_eq!(first.duration, Some(Duration::from_secs(245)));
        assert_eq!(first.artwork.len(), 1);
        assert_eq!(
            first.artwork[0].url,
            "https://yt3.googleusercontent.com/madeup=w544-h544"
        );
        assert_eq!(
            (first.artwork[0].width, first.artwork[0].height),
            (Some(544), Some(544))
        );

        // Two requests, the search first, each a POST to the declared host that the sandbox let through.
        let requests = network.requests();
        assert_eq!(requests.len(), 2, "{requests:?}");
        assert!(requests.iter().all(|r| r.method == Method::Post));
        assert!(requests.iter().all(|r| r.url.host() == "music.youtube.com"));
        assert_eq!(
            requests[0].url.as_str(),
            "https://music.youtube.com/youtubei/v1/search?prettyPrint=false"
        );
        assert_eq!(
            requests[1].url.as_str(),
            "https://music.youtube.com/youtubei/v1/next?prettyPrint=false"
        );
        let search_body = String::from_utf8_lossy(requests[0].body.as_deref().unwrap_or_default());
        assert!(search_body.contains("Song One"), "{search_body}");
        let next_body = String::from_utf8_lossy(requests[1].body.as_deref().unwrap_or_default());
        assert!(next_body.contains("RDAMVMseedvid0001"), "{next_body}");
        assert_eq!(call.report.requests_sent, 2);
        assert_eq!(call.report.requests_refused, 0);
    }

    #[test]
    fn ytmusic_honours_the_count() {
        let network = ScriptedNetwork::new(youtube_music);
        let (host, plugin) = ytmusic_in(&network);
        let tracks = host.radio(&plugin, &seed(), &[], 1).outcome.unwrap();
        assert_eq!(
            tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["one00000001"]
        );
        let none = host.radio(&plugin, &seed(), &[], 0);
        assert_eq!(none.outcome, Ok(Vec::new()));
        // Only the first call sent requests.
        assert_eq!(network.requests().len(), 2);
    }

    #[test]
    fn ytmusic_reports_a_rate_limit_once_and_never_retries() {
        let network = ScriptedNetwork::new(|_| {
            Ok(HttpResponse::new(
                429,
                vec![(String::from("Retry-After"), String::from("7"))],
                Vec::new(),
            ))
        });
        let (host, plugin) = ytmusic_in(&network);
        let call = host.radio(&plugin, &seed(), &[], 5);
        assert_eq!(
            call.outcome,
            Err(PluginFailure::Plugin(PluginError::RateLimited {
                retry_after: Some(Duration::from_secs(7)),
            }))
        );
        // One request: no retry, and no radio request after the failed search.
        assert_eq!(network.requests().len(), 1);
        assert_eq!(call.report.requests_sent, 1);
    }

    #[test]
    fn ytmusic_finds_nothing_for_a_seed_the_service_does_not_have() {
        // The service's only song has another title: the plugin does not take it for the seed.
        let network =
            ScriptedNetwork::new(|_| Ok(json(search_answer("othervid001", "Another Song"))));
        let (host, plugin) = ytmusic_in(&network);
        let call = host.radio(&plugin, &seed(), &[], 5);
        assert_eq!(
            call.outcome,
            Err(PluginFailure::Plugin(PluginError::NotFound))
        );
        assert_eq!(network.requests().len(), 1);
    }

    #[test]
    fn ytmusic_reports_what_it_cannot_read_as_an_upstream_failure() {
        for body in ["not json", "{}", r#"{"contents":{}}"#] {
            let network = ScriptedNetwork::new(move |_| {
                Ok(HttpResponse::new(200, Vec::new(), body.as_bytes().to_vec()))
            });
            let (host, plugin) = ytmusic_in(&network);
            let call = host.radio(&plugin, &seed(), &[], 5);
            assert!(
                matches!(
                    call.outcome,
                    Err(PluginFailure::Plugin(PluginError::Upstream(_)))
                ),
                "{body}: {:?}",
                call.outcome
            );
            assert_eq!(network.requests().len(), 1, "{body}");
        }
    }

    #[test]
    fn ytmusic_does_not_provide_what_it_does_not_declare() {
        let network = ScriptedNetwork::new(youtube_music);
        let (host, plugin) = ytmusic_in(&network);
        let call = host.search(&plugin, "song", &[], None);
        assert_eq!(
            call.outcome.unwrap_err(),
            PluginFailure::NotProvided(Capability::Search)
        );
        assert_eq!(network.requests().len(), 0);
    }
}
