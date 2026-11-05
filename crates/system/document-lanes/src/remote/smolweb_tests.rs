/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use genet_host_api::ResourceFetcher;

use crate::RemoteFetcher;

/// A smolweb scheme is recognized and routed to the errand transport, and a host
/// that cannot resolve fails to a clean `None` (a failed load, not a panic or an
/// error document) -- the same contract the http path holds for a non-2xx. Uses a
/// `.invalid` host (RFC 6761 guarantees NXDOMAIN, answered locally) so the test
/// needs no live capsule, and exercises the one-time TOFU install on the way.
#[test]
fn smolweb_scheme_routes_and_unresolvable_host_is_none() {
    assert!(
        RemoteFetcher::shared()
            .fetch("gemini://capsule.invalid/")
            .is_none(),
        "an unresolvable gemini host is a failed load, not a document"
    );
}

/// A non-smolweb, non-http unknown scheme is not routed to errand; it falls
/// through to the filesystem attempt and fails to `None`.
#[test]
fn unknown_scheme_is_not_routed_to_errand() {
    assert!(
        RemoteFetcher::shared().fetch("wat://nope/").is_none(),
        "a non-smolweb scheme is not an errand fetch nor a readable path"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn submission_targets_are_refused_without_a_connection() {
    use std::{
        io,
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    let fetcher = RemoteFetcher::shared();
    for (scheme, query) in [
        ("titan", ""),
        ("spartan", "?private%20payload"),
        ("spartan", "?"),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!(
            "{scheme}://{}/submit{query}",
            listener.local_addr().unwrap()
        );
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let probe = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut accepted = 0;
            loop {
                match listener.accept() {
                    Ok((socket, _)) => {
                        // Close immediately: a missing admission guard must
                        // fail the assertion instead of hanging in TLS/read.
                        accepted += 1;
                        drop(socket);
                    },
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if worker_stop.load(Ordering::Acquire) || Instant::now() >= deadline {
                            break;
                        }
                        thread::sleep(Duration::from_millis(2));
                    },
                    Err(error) => panic!("submission probe accept failed: {error}"),
                }
            }
            accepted
        });
        let bytes = fetcher.fetch(&url);
        let response = fetcher.fetch_response(&url);
        stop.store(true, Ordering::Release);
        let accepted = probe.join().unwrap();
        assert_eq!(accepted, 0, "a refused read must not reach the transport");
        assert!(bytes.is_none());
        assert!(response.is_none());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn tcp_fixture(
    scheme: &str,
    path: &str,
    expected_request: Vec<u8>,
    response: Vec<u8>,
) -> (String, std::thread::JoinHandle<Vec<u8>>) {
    use std::{
        io::{self, Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("{scheme}://{}{path}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "Protocol fixture was not contacted"
                    );
                    thread::sleep(Duration::from_millis(5));
                },
                Err(error) => panic!("Protocol fixture accept failed: {error}"),
            }
        };
        // Accepted sockets inherit the listener's nonblocking flag on
        // Windows; switch to bounded blocking IO for the fixture exchange.
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n") {
            assert!(request.len() < 1024);
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        assert_eq!(request, expected_request);
        stream.write_all(&response).unwrap();
        request
    });
    (url, worker)
}

#[cfg(not(target_arch = "wasm32"))]
fn spartan_fixture(response: Vec<u8>) -> (String, std::thread::JoinHandle<Vec<u8>>) {
    tcp_fixture(
        "spartan",
        "/saved",
        b"127.0.0.1 /saved 0\r\n".to_vec(),
        response,
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bodyless_finger_response_keeps_native_representation_and_literal_lines() {
    struct CapturingFetch(std::sync::Mutex<Option<genet_host_api::ResourceResponse>>);
    impl ResourceFetcher for CapturingFetch {
        fn fetch(&self, _: &str) -> Option<Vec<u8>> {
            panic!("use the response metadata seam")
        }
        fn fetch_request(
            &self,
            _: &genet_host_api::ResourceRequest,
        ) -> Option<genet_host_api::ResourceFetchResponse> {
            None
        }
        fn fetch_response(&self, url: &str) -> Option<genet_host_api::ResourceResponse> {
            let response = RemoteFetcher::shared().fetch_response(url)?;
            *self.0.lock().unwrap() = Some(response.clone());
            Some(response)
        }
    }
    let body = b"Login: alice   Name: Alice\r\nPlan:\r\n\r\n  * literal\r\n";
    let (url, server) = tcp_fixture("finger", "/alice", b"alice\r\n".to_vec(), body.to_vec());
    let fetcher = CapturingFetch(std::sync::Mutex::new(None));
    let document = crate::SmolwebDocument::load(&fetcher, &url, crate::SmolwebTheme::Plain);
    server.join().unwrap();
    let response = fetcher.0.into_inner().unwrap().unwrap();
    assert_eq!(response.content_type.as_deref(), Some("text/x-finger"));
    assert_eq!(response.bytes, body);
    let document = document.unwrap();
    assert_eq!(
        document.document().provenance.source_kind.as_deref(),
        Some("nematic.finger")
    );
    assert!(matches!(document.document().blocks.as_slice(),
        [inker::Block::Preformatted { text }] if text == "Login: alice   Name: Alice\r\nPlan:\r\n\r\n  * literal"
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queryless_spartan_read_keeps_exact_body_bytes() {
    let body = b"# Saved\r\n\0\xff";
    let mut response = b"2 application/octet-stream\r\n".to_vec();
    response.extend_from_slice(body);
    let (url, server) = spartan_fixture(response);
    let bytes = RemoteFetcher::shared().fetch(&url);
    server.join().unwrap();
    assert_eq!(bytes.unwrap(), body);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queryless_spartan_response_keeps_mime_parameters_and_exact_bytes() {
    let mime = "text/plain; charset=utf-8;lang=fr";
    let body = b"# Opaque\r\n\0\xff";
    let mut wire = format!("2 {mime}\r\n").into_bytes();
    wire.extend_from_slice(body);
    let (url, server) = spartan_fixture(wire);
    let response = RemoteFetcher::shared().fetch_response(&url);
    server.join().unwrap();
    let response = response.unwrap();
    assert_eq!(response.final_url, url);
    assert_eq!(response.content_type.as_deref(), Some(mime));
    assert_eq!(response.bytes, body);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn spartan_redirect_stays_a_failed_load_and_is_not_followed() {
    use std::{io, net::TcpListener};

    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let response = format!(
        "3 spartan://{}/submit?private\r\n",
        target.local_addr().unwrap()
    );
    let (url, server) = spartan_fixture(response.into_bytes());
    let bytes = RemoteFetcher::shared().fetch(&url);
    server.join().unwrap();
    assert!(bytes.is_none());
    assert!(
        matches!(target.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
        "the helper must continue to leave redirects unfollowed"
    );
}
