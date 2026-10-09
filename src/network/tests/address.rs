use crate::network::{address::InvalidServerUrl, is_local_only, server_url, with_host};

#[test]
fn server_urls_are_normalised() {
    let cases = [
        ("192.168.1.20:8080", "ws://192.168.1.20:8080"),
        ("  localhost:9000 ", "ws://localhost:9000"),
        ("ws://race.lan:8080", "ws://race.lan:8080"),
        ("wss://race.example.com", "wss://race.example.com"),
        ("http://race.lan:8080/", "ws://race.lan:8080/"),
        ("HTTPS://race.example.com", "wss://race.example.com"),
        ("ws://[::1]:8080", "ws://[::1]:8080"),
        ("[::1]", "ws://[::1]"),
        ("ws://player@race.lan", "ws://player@race.lan"),
        ("race.lan:65535/rooms?x=1", "ws://race.lan:65535/rooms?x=1"),
    ];
    for (input, expected) in cases {
        assert_eq!(server_url(input).as_deref(), Ok(expected), "{input}");
        assert_eq!(server_url(expected).as_deref(), Ok(expected), "{expected}");
    }
}

#[test]
fn invalid_server_urls_are_rejected() {
    let cases = [
        ("", InvalidServerUrl::Empty),
        ("   ", InvalidServerUrl::Empty),
        ("ftp://race.lan", InvalidServerUrl::Scheme("ftp".to_owned())),
        ("ws://", InvalidServerUrl::Host("ws://".to_owned())),
        (
            "ws://:8080",
            InvalidServerUrl::Host("ws://:8080".to_owned()),
        ),
        (
            "wss:///path",
            InvalidServerUrl::Host("wss:///path".to_owned()),
        ),
        (
            "ws://[]:80",
            InvalidServerUrl::Host("ws://[]:80".to_owned()),
        ),
        (
            "my server:80",
            InvalidServerUrl::Host("my server:80".to_owned()),
        ),
        ("ws://[::1", InvalidServerUrl::Host("ws://[::1".to_owned())),
        (
            "ws://[::1]8080",
            InvalidServerUrl::Host("ws://[::1]8080".to_owned()),
        ),
        ("::1", InvalidServerUrl::Host("::1".to_owned())),
        ("race.lan:", InvalidServerUrl::Port("race.lan:".to_owned())),
        (
            "race.lan:http",
            InvalidServerUrl::Port("race.lan:http".to_owned()),
        ),
        (
            "race.lan:65536",
            InvalidServerUrl::Port("race.lan:65536".to_owned()),
        ),
        (
            "race.lan:+80",
            InvalidServerUrl::Port("race.lan:+80".to_owned()),
        ),
        (
            "ws://[::1]:x",
            InvalidServerUrl::Port("ws://[::1]:x".to_owned()),
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(server_url(input), Err(expected), "{input:?}");
    }
}

#[test]
fn servers_only_this_computer_reaches_are_told_apart() {
    for url in [
        "ws://127.0.0.1:8080",
        "ws://127.1.2.3:8080",
        "ws://localhost:8080",
        "ws://LOCALHOST",
        "ws://race.localhost:9000/rooms",
        "ws://[::1]:8080",
        "ws://0.0.0.0:8080",
        "ws://player@127.0.0.1:8080",
    ] {
        assert!(is_local_only(url), "{url}");
    }
    for url in [
        "ws://192.168.1.42:8080",
        "ws://race.lan:8080",
        "wss://race.example.com",
        "ws://[fe80::1]:8080",
        "ws://localhost.example.com",
    ] {
        assert!(!is_local_only(url), "{url}");
    }
}

#[test]
fn the_host_of_a_server_address_can_be_replaced() {
    let cases = [
        (
            "ws://127.0.0.1:8080",
            "192.168.1.42",
            "ws://192.168.1.42:8080",
        ),
        (
            "wss://localhost/rooms?x=1",
            "10.0.0.9",
            "wss://10.0.0.9/rooms?x=1",
        ),
        ("ws://[::1]:9000", "[fe80::1]", "ws://[fe80::1]:9000"),
        (
            "ws://ada@127.0.0.1:8080",
            "10.0.0.9",
            "ws://ada@10.0.0.9:8080",
        ),
    ];
    for (url, host, expected) in cases {
        assert_eq!(with_host(url, host).as_deref(), Some(expected), "{url}");
    }
    assert_eq!(with_host("127.0.0.1:8080", "10.0.0.9"), None, "no scheme");
}
