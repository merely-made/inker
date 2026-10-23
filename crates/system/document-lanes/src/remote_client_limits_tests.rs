// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn spartan_response(
    body: Vec<u8>,
    hold_open: bool,
) -> (String, mpsc::Sender<()>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let (release, released) = mpsc::channel();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "fixture was not contacted");
                    thread::sleep(Duration::from_millis(2));
                },
                Err(error) => panic!("{error}"),
            }
        };
        // Accepted sockets can inherit the listener's nonblocking mode on Windows.
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut stream = BufReader::new(stream);
        let mut line = String::new();
        stream.read_line(&mut line).unwrap();
        stream.get_mut().write_all(b"2 text/plain\r\n").unwrap();
        stream.get_mut().write_all(&body).unwrap();
        if hold_open {
            // Only the test may release EOF, after the client has returned.
            released.recv_timeout(Duration::from_secs(3)).unwrap();
        } else {
            stream.get_mut().shutdown(Shutdown::Write).unwrap();
        }
    });
    (format!("spartan://{address}/read"), release, server)
}

#[test]
fn bodyless_smolweb_uses_host_body_limits_without_successful_truncation() {
    use inker::session_engine::{SessionEngine, SessionSpawnRequest};
    let policy = ResourceFetchPolicy {
        max_response_bytes: 4,
        timeout: Duration::from_secs(2),
        ..ResourceFetchPolicy::default()
    };
    for extra in [0, 1] {
        let (url, release, server) = spartan_response(vec![b'x'; 4 + extra], extra != 0);
        let engine = crate::SmolwebSessionEngine::new(
            "nematic.spartan",
            RemoteFetcher::new(policy),
            crate::SmolwebTheme::default(),
        );
        let started = Instant::now();
        let result = engine.spawn(&SessionSpawnRequest::new(url));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "cap must abort before EOF or the request deadline"
        );
        if extra == 0 {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
            release.send(()).unwrap();
        }
        server.join().unwrap();
    }
}

#[test]
fn bodyless_smolweb_deadline_covers_an_unfinished_body() {
    let (url, release, server) = spartan_response(Vec::new(), true);
    let policy = ResourceFetchPolicy {
        timeout: Duration::from_millis(100),
        ..ResourceFetchPolicy::default()
    };
    let started = Instant::now();
    assert!(RemoteFetcher::new(policy).fetch(&url).is_none());
    assert!(started.elapsed() < Duration::from_secs(1));
    release.send(()).unwrap();
    server.join().unwrap();
}

#[test]
fn smolweb_queue_wait_uses_the_same_admission_pool_and_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let policy = ResourceFetchPolicy {
        max_concurrent_fetches: 1,
        timeout: Duration::from_millis(100),
        ..ResourceFetchPolicy::default()
    };
    let fetcher = RemoteFetcher::new(policy);
    assert_eq!(fetcher.max_concurrent_fetches(), 1);
    // Occupy the shared host pool as another admitted remote request would.
    let permit = fetcher
        .budget
        .acquire_until(Instant::now() + policy.timeout)
        .unwrap();
    let started = Instant::now();
    assert!(
        fetcher
            .fetch(&format!("spartan://{}/", listener.local_addr().unwrap()))
            .is_none()
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    drop(permit);
    let (url, _release, server) = spartan_response(b"saved".to_vec(), false);
    assert_eq!(fetcher.fetch(&url).unwrap(), b"saved");
    server.join().unwrap();
}

#[test]
fn supplied_empty_body_remains_local_even_when_network_policy_refuses_every_request() {
    use inker::session_engine::{SessionEngine, SessionSpawnRequest};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let fetcher = RemoteFetcher::new(ResourceFetchPolicy {
        max_concurrent_fetches: 0,
        ..ResourceFetchPolicy::default()
    });
    let engine = crate::SmolwebSessionEngine::new(
        "nematic.spartan",
        fetcher,
        crate::SmolwebTheme::default(),
    );
    assert!(
        engine
            .spawn(
                &SessionSpawnRequest::new(format!("spartan://{}/", listener.local_addr().unwrap()))
                    .with_body("")
            )
            .is_ok()
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}
