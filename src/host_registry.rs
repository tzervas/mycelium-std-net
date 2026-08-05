//! `wild:http_request` install into a [`PrimRegistry`] (S-STD-NET / WP-6).
//!
//! # Feature
//!
//! Gated behind **`host-registry`**. Pure client users do not pull `mycelium-interp`.
//!
//! # I/O model — blocking-hypha
//!
//! The host fn **may block** the calling OS thread for the full request (DNS, TLS,
//! body). No reactor in v0 (spike S1).
//!
//! # Value encoding (v0 pragmatic — document widths; zipper bump if changed)
//!
//! | wild name | Args | Result | Encoding |
//! |-----------|------|--------|----------|
//! | `http_request` | `(Bytes, Bytes, Bytes, Bytes, Binary{W})` | `Seq<Bytes>{3}` | see table below |
//!
//! Args: `method`, `url`, `headers` (`name\\tvalue\\n`), `body`, `timeout_ms`
//! (unsigned Binary magnitude; `0` = no timeout).
//!
//! Result sequence elements (homogeneous `Bytes`):
//! 0. status as 2-byte big-endian `u16`
//! 1. response headers (`name\\tvalue\\n`)
//! 2. response body
//!
//! # Guarantee (VR-5 / G2)
//!
//! Ambient network results are tagged **`Declared`** with a zero-magnitude
//! `UserDeclared` error bound. Transport failures never silent-zero a body.
//!
//! # Catalog names
//!
//! Registered under the literal key `wild:http_request`. The prim registry stores
//! host/FFI ops verbatim under the `wild:` namespace (RFC-0028 §4.3); there is no
//! bare-name alias, so the prefix is part of the key.

use mycelium_core::{
    Bound, BoundBasis, BoundKind, GuaranteeStrength, Meta, NormKind, Payload, Provenance, Repr,
    Value,
};
use mycelium_interp::{prims::PrimFn, EvalError, PrimRegistry};

use crate::client::{decode_headers, encode_headers, http_request};

/// Install the v0 net host ops into `reg`.
///
/// Registers:
/// - `wild:http_request`
///
/// Last registration for a name wins ([`PrimRegistry::register`] overwrites). Safe to
/// call after [`PrimRegistry::with_builtins`] and after
/// `mycelium_std_sys_host::install_default_host_ops`.
pub fn install_http_host_ops(reg: &mut PrimRegistry) {
    reg.register("wild:http_request", host_http_request as PrimFn);
}

// --- encoding helpers ---------------------------------------------------------------------------

/// Ambient-host result meta: `Declared` + zero-ε `UserDeclared` bound (M-I4), provenance `Root`.
fn host_declared_meta() -> Result<Meta, EvalError> {
    let bound = Bound {
        kind: BoundKind::Error {
            eps: 0.0,
            norm: NormKind::Linf,
        },
        basis: BoundBasis::UserDeclared,
    };
    Meta::new(
        Provenance::Root,
        GuaranteeStrength::Declared,
        Some(bound),
        None,
        None,
        None,
    )
    .map_err(EvalError::Wf)
}

fn expect_arity(prim: &str, args: &[&Value], n: usize) -> Result<(), EvalError> {
    if args.len() == n {
        Ok(())
    } else {
        Err(EvalError::PrimType {
            prim: prim.to_owned(),
            why: format!("expected {n} argument(s), got {}", args.len()),
        })
    }
}

fn expect_bytes(prim: &str, role: &str, v: &Value) -> Result<Vec<u8>, EvalError> {
    match (v.repr(), v.payload()) {
        (Repr::Bytes, Payload::Bytes(b)) => Ok(b.clone()),
        _ => Err(EvalError::PrimType {
            prim: prim.to_owned(),
            why: format!("expected Bytes for {role}"),
        }),
    }
}

/// Unsigned magnitude of a `Binary{W}` (MSB-first), checked — never wrap (G2).
fn binary_as_u64(prim: &str, v: &Value) -> Result<u64, EvalError> {
    let bits = match (v.repr(), v.payload()) {
        (Repr::Binary { .. }, Payload::Bits(b)) => b.as_slice(),
        _ => {
            return Err(EvalError::PrimType {
                prim: prim.to_owned(),
                why: "expected a Binary timeout_ms operand".to_owned(),
            });
        }
    };
    let mut n: u128 = 0;
    for &b in bits {
        n = n
            .checked_shl(1)
            .and_then(|x| x.checked_add(u128::from(b)))
            .ok_or_else(|| EvalError::PrimType {
                prim: prim.to_owned(),
                why: "Binary timeout_ms magnitude overflowed u128".to_owned(),
            })?;
    }
    u64::try_from(n).map_err(|_| EvalError::PrimType {
        prim: prim.to_owned(),
        why: format!("Binary timeout_ms {n} does not fit u64"),
    })
}

fn bytes_value(bytes: Vec<u8>) -> Result<Value, EvalError> {
    Value::new(Repr::Bytes, Payload::Bytes(bytes), host_declared_meta()?).map_err(EvalError::Wf)
}

fn utf8_str(prim: &str, role: &str, bytes: &[u8]) -> Result<String, EvalError> {
    std::str::from_utf8(bytes)
        .map(|s| s.to_owned())
        .map_err(|e| EvalError::PrimType {
            prim: prim.to_owned(),
            why: format!("{role} is not UTF-8: {e}"),
        })
}

// --- host op ------------------------------------------------------------------------------------

/// `wild:http_request : (Bytes, Bytes, Bytes, Bytes, Binary{W}) → Seq<Bytes>{3}`
fn host_http_request(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    expect_arity(prim, args, 5)?;

    let method_b = expect_bytes(prim, "method", args[0])?;
    let url_b = expect_bytes(prim, "url", args[1])?;
    let headers_b = expect_bytes(prim, "headers", args[2])?;
    let body_b = expect_bytes(prim, "body", args[3])?;
    let timeout_ms = binary_as_u64(prim, args[4])?;

    let method = utf8_str(prim, "method", &method_b)?;
    let url = utf8_str(prim, "url", &url_b)?;
    let headers = decode_headers(&headers_b).map_err(|e| EvalError::PrimType {
        prim: prim.to_owned(),
        why: e.to_string(),
    })?;

    let timeout = if timeout_ms == 0 {
        None
    } else {
        Some(timeout_ms)
    };

    let resp = http_request(&method, &url, &headers, &body_b, timeout).map_err(|e| {
        EvalError::PrimType {
            prim: prim.to_owned(),
            why: e.to_string(),
        }
    })?;

    // status as 2-byte BE
    let status_bytes = resp.status.to_be_bytes().to_vec();
    let header_bytes = encode_headers(&resp.headers);

    let elems = vec![
        bytes_value(status_bytes)?,
        bytes_value(header_bytes)?,
        bytes_value(resp.body)?,
    ];

    Value::new(
        Repr::Seq {
            elem: Box::new(Repr::Bytes),
            len: 3,
        },
        Payload::Seq(elems),
        host_declared_meta()?,
    )
    .map_err(EvalError::Wf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mycelium_core::{binary, Meta, Payload, Provenance, Repr, Value};
    use mycelium_interp::PrimRegistry;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn bytes_val(b: &[u8]) -> Value {
        Value::new(
            Repr::Bytes,
            Payload::Bytes(b.to_vec()),
            Meta::exact(Provenance::Root),
        )
        .expect("wf")
    }

    fn bin_u64(n: u64, width: u32) -> Value {
        let bits = binary::uint_to_bits(n, width).expect("fits");
        Value::new(
            Repr::Binary { width },
            Payload::Bits(bits),
            Meta::exact(Provenance::Root),
        )
        .expect("wf")
    }

    fn spawn_echo_server() -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let url = format!("http://{addr}/echo");
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
            let body_owned = if let Some(pos) = header_end {
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
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\nX-Echo: yes\r\n\r\n",
                body_owned.len()
            );
            stream.write_all(resp.as_bytes()).expect("write head");
            stream.write_all(&body_owned).expect("write body");
            let _ = stream.flush();
        });
        (url, handle)
    }

    #[test]
    fn install_registers_catalog_name() {
        let mut reg = PrimRegistry::with_builtins();
        assert!(!reg.has_host("http_request"));
        assert!(!reg.has_host("wild:http_request"));

        install_http_host_ops(&mut reg);

        assert!(reg.has_host("http_request"));
        assert!(reg.has_host("wild:http_request"));
        assert!(reg.names().contains(&"wild:http_request"));
    }

    #[test]
    fn arity_errors_are_explicit() {
        let mut reg = PrimRegistry::empty();
        install_http_host_ops(&mut reg);
        let f = reg.get("wild:http_request").unwrap();
        let err = f("wild:http_request", &[]).unwrap_err();
        assert!(matches!(err, EvalError::PrimType { .. }));
    }

    #[test]
    fn host_http_request_local_post_roundtrip() {
        let (url, handle) = spawn_echo_server();
        let mut reg = PrimRegistry::empty();
        install_http_host_ops(&mut reg);
        let f = reg.get("wild:http_request").expect("registered");

        let method = bytes_val(b"POST");
        let url_v = bytes_val(url.as_bytes());
        let headers = bytes_val(b"Content-Type\ttext/plain\n");
        let body = bytes_val(b"hello-wild");
        let timeout = bin_u64(5_000, 32);

        let v = f(
            "wild:http_request",
            &[&method, &url_v, &headers, &body, &timeout],
        )
        .expect("request ok");

        assert_eq!(
            *v.repr(),
            Repr::Seq {
                elem: Box::new(Repr::Bytes),
                len: 3
            }
        );
        let Payload::Seq(elems) = v.payload() else {
            panic!("expected Seq payload");
        };
        assert_eq!(elems.len(), 3);

        let status_bytes = elems[0].bytes().expect("status bytes");
        assert_eq!(status_bytes, &[0x00, 0xc8]); // 200
        assert_eq!(elems[2].bytes().expect("body"), b"hello-wild");

        // Declared guarantee on ambient result
        assert_eq!(v.meta().guarantee(), GuaranteeStrength::Declared);

        handle.join().expect("server thread");
    }

    #[test]
    fn host_rejects_non_bytes_method() {
        let mut reg = PrimRegistry::empty();
        install_http_host_ops(&mut reg);
        let f = reg.get("wild:http_request").unwrap();
        let not_bytes = bin_u64(1, 8);
        let url = bytes_val(b"http://127.0.0.1/");
        let empty = bytes_val(b"");
        let timeout = bin_u64(100, 16);
        let err = f(
            "wild:http_request",
            &[&not_bytes, &url, &empty, &empty, &timeout],
        )
        .unwrap_err();
        match err {
            EvalError::PrimType { why, .. } => assert!(why.contains("Bytes"), "{why}"),
            other => panic!("expected PrimType, got {other:?}"),
        }
    }
}
