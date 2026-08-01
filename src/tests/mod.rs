//! Integration tests for mycelium-std-net (logic files stay test-free where possible).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use crate::{http_request, NetError, GUARANTEE_MATRIX};

fn spawn_http_server(
    status_line: &'static str,
    body: &'static [u8],
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let url = format!("http://{addr}/");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buf = [0u8; 8192];
        let _ = stream.read(&mut buf);
        let resp = format!(
            "{status_line}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(resp.as_bytes()).expect("head");
        stream.write_all(body).expect("body");
        let _ = stream.flush();
    });
    (url, handle)
}

#[test]
fn local_get_200() {
    let (url, handle) = spawn_http_server("HTTP/1.1 200 OK", b"pong");
    let resp = http_request("GET", &url, &[], b"", Some(5_000)).expect("GET");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"pong");
    handle.join().unwrap();
}

#[test]
fn local_post_echo_body() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let url = format!("http://{addr}/post");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        // Read until headers complete, then Content-Length body (TCP may split).
        let mut raw = Vec::new();
        let mut tmp = [0u8; 1024];
        let header_end = loop {
            let n = stream.read(&mut tmp).expect("read");
            if n == 0 {
                break None;
            }
            raw.extend_from_slice(&tmp[..n]);
            if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                break Some(pos);
            }
            if raw.len() > 64 * 1024 {
                break None;
            }
        };
        let body = if let Some(pos) = header_end {
            let headers = std::str::from_utf8(&raw[..pos]).unwrap_or("");
            let mut content_len = 0usize;
            for line in headers.lines() {
                let lower = line.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("content-length:") {
                    content_len = rest.trim().parse().unwrap_or(0);
                }
            }
            let mut body = raw[pos + 4..].to_vec();
            while body.len() < content_len {
                let n = stream.read(&mut tmp).expect("read body");
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&tmp[..n]);
            }
            body.truncate(content_len);
            body
        } else {
            Vec::new()
        };
        let head = format!(
            "HTTP/1.1 201 Created\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
    });

    let payload = b"{\"ok\":true}";
    let resp = http_request(
        "POST",
        &url,
        &[("Content-Type".into(), "application/json".into())],
        payload,
        Some(5_000),
    )
    .expect("POST");
    assert_eq!(resp.status, 201);
    assert_eq!(resp.body, payload);
    handle.join().unwrap();
}

#[test]
fn local_404_is_success_with_status() {
    let (url, handle) = spawn_http_server("HTTP/1.1 404 Not Found", b"missing");
    let resp = http_request("GET", &url, &[], b"", Some(5_000)).expect("GET 404");
    assert_eq!(resp.status, 404);
    assert_eq!(resp.body, b"missing");
    handle.join().unwrap();
}

#[test]
fn connect_refused_is_explicit_transport_err() {
    // Port with nothing listening — never silent empty body.
    let err = http_request("GET", "http://127.0.0.1:1/", &[], b"", Some(500)).unwrap_err();
    assert!(
        matches!(err, NetError::Transport { .. }),
        "expected Transport, got {err:?}"
    );
}

#[test]
fn guarantee_matrix_covers_http_request() {
    assert!(GUARANTEE_MATRIX.iter().any(|r| r.op == "http_request"));
    assert!(GUARANTEE_MATRIX.iter().all(|r| r.guarantee == "Declared"));
}

/// Live HTTPS smoke against a public host. Opt-in: `cargo test -- --ignored`.
#[test]
#[ignore = "requires outbound network"]
fn live_https_get_example() {
    let resp = http_request(
        "GET",
        "https://example.com/",
        &[("User-Agent".into(), "mycelium-std-net/0.464".into())],
        b"",
        Some(15_000),
    )
    .expect("HTTPS GET example.com");
    assert!(
        resp.status >= 200 && resp.status < 400,
        "unexpected status {}",
        resp.status
    );
    assert!(!resp.body.is_empty(), "expected non-empty HTML body");
}
