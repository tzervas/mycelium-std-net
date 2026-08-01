//! Blocking HTTP(S) client surface over ureq + rustls.
//!
//! # I/O model — blocking-hypha
//!
//! Every call **may block** the calling OS thread for the full request lifetime
//! (DNS, TCP, TLS handshake, body transfer). No reactor in v0 (spike S1).
//!
//! # TLS
//!
//! ureq default features enable **rustls** with webpki roots (spike S3). No
//! native-tls / OpenSSL path in this crate.

use std::time::Duration;

use ureq::http::{Method, Request};
use ureq::Agent;

use crate::error::NetError;

/// Successful HTTP response (status may be 4xx/5xx — caller decides).
///
/// **Never-silent:** a transport failure is [`NetError`], not a zero-status /
/// empty-body “success”.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code (100–599 when the peer spoke HTTP).
    pub status: u16,
    /// Response header pairs in arrival order (duplicates preserved).
    pub headers: Vec<(String, String)>,
    /// Full response body (subject to ureq’s default body size limit).
    pub body: Vec<u8>,
}

/// Perform one blocking HTTP(S) request.
///
/// # Arguments
///
/// * `method` — case-sensitive method token (`GET`, `POST`, `PUT`, `DELETE`, …).
/// * `url` — absolute `http://` or `https://` URL.
/// * `headers` — request headers; empty name is refused.
/// * `body` — request body bytes (sent when non-empty; empty uses no-body mode).
/// * `timeout_ms` — optional global timeout for the call; `None` or `Some(0)` =
///   no explicit timeout (ureq defaults).
///
/// # Errors
///
/// Invalid inputs and transport failures return [`NetError`]. HTTP error statuses
/// (4xx/5xx) are **not** errors — they populate [`HttpResponse::status`].
pub fn http_request(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
    timeout_ms: Option<u64>,
) -> Result<HttpResponse, NetError> {
    validate_method(method)?;
    validate_url(url)?;
    for (n, v) in headers {
        validate_header(n, v)?;
    }

    let http_method =
        Method::from_bytes(method.as_bytes()).map_err(|e| NetError::InvalidMethod {
            why: format!("not a valid HTTP method token: {e}"),
        })?;

    let agent = build_agent(timeout_ms);

    let mut builder = Request::builder().method(http_method).uri(url);
    for (name, value) in headers {
        builder = builder.header(name.as_str(), value.as_str());
    }

    // Always attach the body slice (empty = no payload). Single type path for agent.run.
    let request = builder.body(body).map_err(|e| NetError::Protocol {
        why: format!("failed to build request: {e}"),
    })?;

    // http_status_as_error(false): 4xx/5xx are data, not Err (G2 — caller sees status).
    let request = agent
        .configure_request(request)
        .http_status_as_error(false)
        .build();

    let mut response = agent.run(request).map_err(NetError::from)?;

    let status = response.status().as_u16();
    let mut out_headers = Vec::new();
    for (name, value) in response.headers().iter() {
        out_headers.push((
            name.as_str().to_owned(),
            value.to_str().unwrap_or("").to_owned(),
        ));
    }

    let body = response
        .body_mut()
        .read_to_vec()
        .map_err(|e| NetError::BodyRead { why: e.to_string() })?;

    Ok(HttpResponse {
        status,
        headers: out_headers,
        body,
    })
}

fn build_agent(timeout_ms: Option<u64>) -> Agent {
    let mut builder = Agent::config_builder();
    if let Some(ms) = timeout_ms {
        if ms > 0 {
            builder = builder.timeout_global(Some(Duration::from_millis(ms)));
        }
    }
    builder.build().into()
}

fn validate_method(method: &str) -> Result<(), NetError> {
    if method.is_empty() {
        return Err(NetError::InvalidMethod {
            why: "method must be non-empty".to_owned(),
        });
    }
    // RFC 9110 token: visible ASCII without separators — keep light for v0.
    if !method.bytes().all(|b| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'!' | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'.'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'|'
                    | b'~'
            )
    }) {
        return Err(NetError::InvalidMethod {
            why: format!("method {method:?} is not an HTTP token"),
        });
    }
    Ok(())
}

fn validate_url(url: &str) -> Result<(), NetError> {
    if url.is_empty() {
        return Err(NetError::InvalidUrl {
            why: "url must be non-empty".to_owned(),
        });
    }
    let lower = url.as_bytes();
    let http = lower.starts_with(b"http://");
    let https = lower.starts_with(b"https://");
    if !http && !https {
        return Err(NetError::InvalidUrl {
            why: "url must start with http:// or https://".to_owned(),
        });
    }
    Ok(())
}

fn validate_header(name: &str, value: &str) -> Result<(), NetError> {
    if name.is_empty() {
        return Err(NetError::InvalidHeader {
            why: "header name must be non-empty".to_owned(),
        });
    }
    if name.bytes().any(|b| b == b'\r' || b == b'\n' || b == b'\0') {
        return Err(NetError::InvalidHeader {
            why: "header name must not contain CR/LF/NUL".to_owned(),
        });
    }
    if value
        .bytes()
        .any(|b| b == b'\r' || b == b'\n' || b == b'\0')
    {
        return Err(NetError::InvalidHeader {
            why: format!("header {name:?} value must not contain CR/LF/NUL"),
        });
    }
    Ok(())
}

/// Encode headers as `name\\tvalue\\n` lines (host-registry / wire helper).
pub fn encode_headers(headers: &[(String, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (n, v) in headers {
        out.extend_from_slice(n.as_bytes());
        out.push(b'\t');
        out.extend_from_slice(v.as_bytes());
        out.push(b'\n');
    }
    out
}

/// Decode `name\\tvalue\\n` header lines. Malformed lines are refused (G2).
pub fn decode_headers(bytes: &[u8]) -> Result<Vec<(String, String)>, NetError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| NetError::InvalidHeader {
        why: format!("headers are not UTF-8: {e}"),
    })?;
    let mut out = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once('\t')
            .ok_or_else(|| NetError::InvalidHeader {
                why: format!("header line {i} missing tab separator"),
            })?;
        validate_header(name, value)?;
        out.push((name.to_owned(), value.to_owned()));
    }
    Ok(out)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn reject_empty_method() {
        let err = http_request("", "http://127.0.0.1/", &[], b"", Some(100)).unwrap_err();
        assert!(matches!(err, NetError::InvalidMethod { .. }));
    }

    #[test]
    fn reject_bad_url_scheme() {
        let err = http_request("GET", "ftp://example.com/", &[], b"", Some(100)).unwrap_err();
        assert!(matches!(err, NetError::InvalidUrl { .. }));
    }

    #[test]
    fn reject_empty_header_name() {
        let err = http_request(
            "GET",
            "http://127.0.0.1/",
            &[("".into(), "x".into())],
            b"",
            Some(100),
        )
        .unwrap_err();
        assert!(matches!(err, NetError::InvalidHeader { .. }));
    }

    #[test]
    fn header_codec_roundtrip() {
        let hs = vec![
            ("Accept".into(), "text/plain".into()),
            ("X-Test".into(), "1".into()),
        ];
        let enc = encode_headers(&hs);
        let dec = decode_headers(&enc).expect("decode");
        assert_eq!(dec, hs);
    }

    #[test]
    fn header_codec_empty() {
        assert!(decode_headers(b"").unwrap().is_empty());
        assert!(encode_headers(&[]).is_empty());
    }

    #[test]
    fn header_codec_rejects_missing_tab() {
        let err = decode_headers(b"NoTabHere\n").unwrap_err();
        assert!(matches!(err, NetError::InvalidHeader { .. }));
    }
}
