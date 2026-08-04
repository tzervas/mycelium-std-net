//! `prim:http_request` / `prim:http_get` — the checked, non-`wild` HTTP surface
//! (S-STD-NET-SAFE-HTTP, PKG-LINKAGE, mycelium-lang#44).
//!
//! ## Why this exists
//!
//! `wild:http_request` (see [`crate::host_registry`]) is real and works (verified: a live
//! HTTPS 200 from a public host via ureq+rustls), but it goes through `wild`'s
//! ascription-on-faith boundary: a `.myc` `fn` may declare any result type it likes and both
//! `myc check` and `myc run` report success even if that ascription does not match what the op
//! actually returns (measured defect #4 — exactly the wrong property for an HTTP client). This
//! module adds a second, disjoint `prim:` door whose signature — arity, per-argument
//! [`TySpec`](mycelium_interp::typed::TySpec), result type, and declared effects — is
//! `myc check`-verifiable against a REGISTERED
//! [`PrimSig`](mycelium_interp::typed::PrimSig), instead of trusted on ascription.
//!
//! `wild:http_request` / [`crate::host_registry::install_http_host_ops`] are byte-for-byte
//! unchanged by this file (PKG-LINKAGE non-goal: `wild` stays the audited, ascription-trusted
//! floor-authoring escape hatch).
//!
//! ## Split from the ureq/rustls stack (feature `typed-prims` vs. feature `client`)
//!
//! [`typed_prim_sigs`] returns pure [`PrimSig`](mycelium_interp::typed::PrimSig) data — no
//! TLS/network stack — so a static `myc check` build can verify a `.myc` caller's typed-prim
//! call sites against these signatures without linking `ureq`/`rustls` at all. Measured:
//! `cargo tree -e normal --no-default-features --features typed-prims` shows no `ureq`/`rustls`
//! edge (see this crate's `Cargo.toml` feature docs). The actual dispatch
//! ([`install_typed_http_prims`], gated additionally on feature `client`) needs the same
//! ureq+rustls blocking client [`crate::client::http_request`] already uses for
//! `wild:http_request`, to perform a real request.
//!
//! ## Checked result type
//!
//! Both prims resolve to [`HTTP_RESPONSE_ADT`] (`"std.net.HttpResponse"`) — a
//! `.myc`-nameable [`TySpec::Adt`](mycelium_interp::typed::TySpec::Adt) — instead of the `wild`
//! path's opaque, ascription-trusted `Seq<Bytes>{3}`. `myc check` verifies a caller's declared
//! result type against this REGISTERED [`PrimSig`], rather than trusting whatever a `.myc`
//! author ascribes to a `wild` call. The on-the-wire runtime encoding underneath is
//! intentionally kept byte-identical to the `wild` path's own status/headers/body triple (see
//! [`crate::host_registry`]'s doc table): this surface adds a checked *type*, not a second wire
//! format for the same bytes — reusing a proven encoding rather than inventing one.
//!
//! ## Effect: `"net"`, not `"ffi"`
//!
//! Both [`PrimSig`](mycelium_interp::typed::PrimSig)s declare `effects: vec!["net".into()]` — a
//! caller must cover `!{net}` specifically, never the blanket `!{ffi}` (proves the effect name
//! is taken from this registered signature, not hardcoded — PKG-LINKAGE adversarial-review
//! item).

use mycelium_core::GuaranteeStrength;
#[cfg(feature = "client")]
use mycelium_core::{Bound, BoundBasis, BoundKind, Meta, NormKind, Provenance};
use mycelium_interp::typed::{PrimSig, TySpec, WidthSpec};

/// The checked HTTP response ADT name both prims resolve to (S-STD-NET-SAFE-HTTP). A
/// `.myc`-nameable constructed type name per [`TySpec::Adt`]'s own contract — never a
/// Rust-internal type name (PKG-LINKAGE self-hosting-scope-leak guard).
pub const HTTP_RESPONSE_ADT: &str = "std.net.HttpResponse";

/// `prim:http_request` — full control (method/url/headers/body/timeout), mirrors
/// `wild:http_request`'s arg shape (see [`crate::host_registry`]'s doc table) at the checked
/// layer.
#[must_use]
pub fn http_request_sig() -> PrimSig {
    PrimSig {
        name: "std.net.http.http_request".to_owned(),
        params: vec![
            TySpec::Bytes,                 // method (UTF-8 HTTP token, e.g. "GET"/"POST")
            TySpec::Bytes,                 // url (UTF-8 absolute http:// or https:// URL)
            TySpec::Bytes,                 // headers ("name\tvalue\n" lines; empty = none)
            TySpec::Bytes,                 // body (may be empty)
            TySpec::Binary(WidthSpec(64)), // timeout_ms (unsigned magnitude; 0 = no timeout)
        ],
        ret: TySpec::Adt(HTTP_RESPONSE_ADT.to_owned()),
        effects: vec!["net".to_owned()],
        guarantee: GuaranteeStrength::Declared,
    }
}

/// `prim:http_get` — convenience GET (no method/body arguments; method is fixed `"GET"`).
#[must_use]
pub fn http_get_sig() -> PrimSig {
    PrimSig {
        name: "std.net.http.http_get".to_owned(),
        params: vec![
            TySpec::Bytes,                 // url
            TySpec::Bytes,                 // headers
            TySpec::Binary(WidthSpec(64)), // timeout_ms
        ],
        ret: TySpec::Adt(HTTP_RESPONSE_ADT.to_owned()),
        effects: vec!["net".to_owned()],
        guarantee: GuaranteeStrength::Declared,
    }
}

/// Pure signature data for both checked HTTP prims (S-STD-NET-SAFE-HTTP). No `ureq`/`rustls`
/// edge — a checker (`mycelium-l1`, a later PKG-LINKAGE lane) resolves `use std_net::http...`
/// call sites against these without this crate's `client` feature ever being enabled.
#[must_use]
pub fn typed_prim_sigs() -> Vec<PrimSig> {
    vec![http_request_sig(), http_get_sig()]
}

// --- dispatch (the real request — needs the ureq+rustls client, feature `client`) ---------------

#[cfg(feature = "client")]
mod dispatch {
    use mycelium_core::{Payload, Repr, Value};
    use mycelium_interp::prims::PrimFn;
    use mycelium_interp::typed::{install_typed_prims, TypedPrimRegistry};
    use mycelium_interp::EvalError;

    use crate::client::{decode_headers, encode_headers, http_request, HttpResponse};

    use super::{declared_meta, http_get_sig, http_request_sig};

    /// Install `prim:http_request` / `prim:http_get` into `reg` (S-TYPED-PRIM-REGISTRY). Needs
    /// feature `client` for the actual blocking request; [`super::typed_prim_sigs`] alone (no
    /// `client`) is sufficient for a checker that only needs the signatures.
    pub fn install_typed_http_prims(reg: &mut TypedPrimRegistry) {
        install_typed_prims(
            reg,
            &[
                (
                    "http_request",
                    http_request_sig(),
                    typed_http_request as PrimFn,
                ),
                ("http_get", http_get_sig(), typed_http_get as PrimFn),
            ],
        );
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
        Value::new(Repr::Bytes, Payload::Bytes(bytes), declared_meta()?).map_err(EvalError::Wf)
    }

    fn utf8_str(prim: &str, role: &str, bytes: &[u8]) -> Result<String, EvalError> {
        std::str::from_utf8(bytes)
            .map(|s| s.to_owned())
            .map_err(|e| EvalError::PrimType {
                prim: prim.to_owned(),
                why: format!("{role} is not UTF-8: {e}"),
            })
    }

    /// Encode a real [`HttpResponse`] as the checked response value: the SAME status/headers/body
    /// `Seq<Bytes>{3}` triple `wild:http_request` already emits (see
    /// [`crate::host_registry::install_http_host_ops`]'s doc table) — this module checks the
    /// *declared type* ([`super::HTTP_RESPONSE_ADT`]), not a new wire encoding.
    fn response_value(resp: HttpResponse) -> Result<Value, EvalError> {
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
            declared_meta()?,
        )
        .map_err(EvalError::Wf)
    }

    /// `prim:http_request : (Bytes, Bytes, Bytes, Bytes, Binary{64}) -> Adt("std.net.HttpResponse")`.
    fn typed_http_request(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
        expect_arity(prim, args, 5)?;
        let method = utf8_str(prim, "method", &expect_bytes(prim, "method", args[0])?)?;
        let url = utf8_str(prim, "url", &expect_bytes(prim, "url", args[1])?)?;
        let headers_b = expect_bytes(prim, "headers", args[2])?;
        let body_b = expect_bytes(prim, "body", args[3])?;
        let timeout_ms = binary_as_u64(prim, args[4])?;

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
        response_value(resp)
    }

    /// `prim:http_get : (Bytes, Bytes, Binary{64}) -> Adt("std.net.HttpResponse")`.
    fn typed_http_get(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
        expect_arity(prim, args, 3)?;
        let url = utf8_str(prim, "url", &expect_bytes(prim, "url", args[0])?)?;
        let headers_b = expect_bytes(prim, "headers", args[1])?;
        let timeout_ms = binary_as_u64(prim, args[2])?;

        let headers = decode_headers(&headers_b).map_err(|e| EvalError::PrimType {
            prim: prim.to_owned(),
            why: e.to_string(),
        })?;
        let timeout = if timeout_ms == 0 {
            None
        } else {
            Some(timeout_ms)
        };

        let resp =
            http_request("GET", &url, &headers, b"", timeout).map_err(|e| EvalError::PrimType {
                prim: prim.to_owned(),
                why: e.to_string(),
            })?;
        response_value(resp)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use mycelium_core::{
            binary, GuaranteeStrength, Meta as CoreMeta, Provenance as CoreProvenance,
        };
        use mycelium_interp::typed::{TySpec, TypedPrimRegistry};
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        fn bytes_val(b: &[u8]) -> Value {
            Value::new(
                Repr::Bytes,
                Payload::Bytes(b.to_vec()),
                CoreMeta::exact(CoreProvenance::Root),
            )
            .expect("wf")
        }

        fn bin_u64(n: u64, width: u32) -> Value {
            let bits = binary::uint_to_bits(n, width).expect("fits");
            Value::new(
                Repr::Binary { width },
                Payload::Bits(bits),
                CoreMeta::exact(CoreProvenance::Root),
            )
            .expect("wf")
        }

        fn spawn_echo_server() -> (String, thread::JoinHandle<()>) {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let url = format!("http://{addr}/echo");
            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept");
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
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n",
                    body_owned.len()
                );
                stream.write_all(resp.as_bytes()).expect("write head");
                stream.write_all(&body_owned).expect("write body");
                let _ = stream.flush();
            });
            (url, handle)
        }

        #[test]
        fn install_registers_prim_prefix_names() {
            let mut reg = TypedPrimRegistry::empty();
            assert!(!reg.has_typed("http_request"));
            assert!(!reg.has_typed("prim:http_get"));

            install_typed_http_prims(&mut reg);

            assert!(reg.has_typed("http_request"));
            assert!(reg.has_typed("prim:http_request"));
            assert!(reg.has_typed("http_get"));
            assert!(reg.has_typed("prim:http_get"));
            assert_eq!(reg.sigs().count(), 2);
        }

        #[test]
        fn typed_http_request_local_post_roundtrip() {
            let (url, handle) = spawn_echo_server();
            let mut reg = TypedPrimRegistry::empty();
            install_typed_http_prims(&mut reg);
            let (sig, f) = reg.get_typed("prim:http_request").expect("registered");
            assert_eq!(
                sig.ret,
                TySpec::Adt(super::super::HTTP_RESPONSE_ADT.to_owned())
            );
            assert_eq!(sig.effects, vec!["net".to_owned()]);

            let method = bytes_val(b"POST");
            let url_v = bytes_val(url.as_bytes());
            let headers = bytes_val(b"Content-Type\ttext/plain\n");
            let body = bytes_val(b"hello-typed");
            let timeout = bin_u64(5_000, 64);

            let v = f(
                "prim:http_request",
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
            assert_eq!(elems[2].bytes().expect("body"), b"hello-typed");
            assert_eq!(v.meta().guarantee(), GuaranteeStrength::Declared);

            handle.join().expect("server thread");
        }

        #[test]
        fn typed_http_get_local_roundtrip() {
            let (url, handle) = spawn_echo_server();
            let mut reg = TypedPrimRegistry::empty();
            install_typed_http_prims(&mut reg);
            let f = reg.get_typed("prim:http_get").expect("registered").1;

            let url_v = bytes_val(url.as_bytes());
            let headers = bytes_val(b"");
            let timeout = bin_u64(5_000, 64);

            let v = f("prim:http_get", &[&url_v, &headers, &timeout]).expect("GET ok");
            let Payload::Seq(elems) = v.payload() else {
                panic!("expected Seq payload");
            };
            assert_eq!(elems[0].bytes().expect("status bytes"), &[0x00, 0xc8]);

            handle.join().expect("server thread");
        }

        #[test]
        fn typed_http_request_rejects_wrong_arity() {
            let mut reg = TypedPrimRegistry::empty();
            install_typed_http_prims(&mut reg);
            let f = reg.get_typed("prim:http_request").expect("registered").1;
            let err = f("prim:http_request", &[]).unwrap_err();
            assert!(matches!(err, EvalError::PrimType { .. }));
        }

        /// Differential check against the existing `wild:http_request` fixture
        /// ([`crate::host_registry`]): same public host, same expected status range. Opt-in
        /// (`cargo test -- --ignored`) — needs outbound network, mirrors this crate's existing
        /// `tests::live_https_get_example` opt-in live test.
        #[cfg(feature = "host-registry")]
        #[test]
        #[ignore = "requires outbound network"]
        fn typed_and_wild_agree_on_status_for_same_host() {
            use crate::host_registry::install_http_host_ops;
            use mycelium_interp::PrimRegistry;

            let host = "https://example.com/";

            // typed (prim:) path
            let mut treg = TypedPrimRegistry::empty();
            install_typed_http_prims(&mut treg);
            let tf = treg.get_typed("prim:http_get").expect("registered").1;
            let url_v = bytes_val(host.as_bytes());
            let headers = bytes_val(b"");
            let timeout = bin_u64(15_000, 64);
            let tv = tf("prim:http_get", &[&url_v, &headers, &timeout]).expect("typed GET");
            let Payload::Seq(telems) = tv.payload() else {
                panic!("expected Seq payload");
            };
            let typed_status = u16::from_be_bytes(
                telems[0]
                    .bytes()
                    .expect("status bytes")
                    .try_into()
                    .expect("2 bytes"),
            );

            // wild (wild:) path
            let mut wreg = PrimRegistry::empty();
            install_http_host_ops(&mut wreg);
            let wf = wreg.get("wild:http_request").expect("registered");
            let method = bytes_val(b"GET");
            let empty = bytes_val(b"");
            let wv = wf(
                "wild:http_request",
                &[&method, &url_v, &empty, &empty, &timeout],
            )
            .expect("wild GET");
            let Payload::Seq(welems) = wv.payload() else {
                panic!("expected Seq payload");
            };
            let wild_status = u16::from_be_bytes(
                welems[0]
                    .bytes()
                    .expect("status bytes")
                    .try_into()
                    .expect("2 bytes"),
            );

            assert_eq!(
                typed_status, wild_status,
                "typed and wild paths must observe the same status for the same host"
            );
            assert!(
                (200..400).contains(&typed_status),
                "unexpected status {typed_status}"
            );
        }
    }
}

#[cfg(feature = "client")]
pub use dispatch::install_typed_http_prims;

/// Ambient-host result meta: `Declared` + zero-ε `UserDeclared` bound (mirrors
/// [`crate::host_registry`]'s own private `host_declared_meta`), provenance `Root`. Used by
/// [`dispatch`]'s response encoding.
#[cfg(feature = "client")]
fn declared_meta() -> Result<Meta, mycelium_interp::EvalError> {
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
    .map_err(mycelium_interp::EvalError::Wf)
}

#[cfg(test)]
mod sig_tests {
    use super::*;

    /// `typed_prim_sigs()` alone (no `client`, no `host-registry`) is compilable and returns
    /// exactly the two registered prims, each with the `"net"` effect and the checked
    /// [`HTTP_RESPONSE_ADT`] result type — never the blanket `"ffi"`, never the wild path's
    /// opaque `Seq<Bytes>{3}` (PKG-LINKAGE success criterion / adversarial-review item).
    #[test]
    fn typed_prim_sigs_is_nonempty_with_net_effect_and_checked_adt() {
        let sigs = typed_prim_sigs();
        assert_eq!(sigs.len(), 2, "expected http_request + http_get");
        for s in &sigs {
            assert_eq!(s.effects, vec!["net".to_owned()], "sig: {s:?}");
            assert_eq!(
                s.ret,
                TySpec::Adt(HTTP_RESPONSE_ADT.to_owned()),
                "sig: {s:?}"
            );
            assert_eq!(s.guarantee, GuaranteeStrength::Declared, "sig: {s:?}");
        }
        assert!(
            sigs.iter().any(|s| s.params.len() == 5),
            "http_request should take 5 args"
        );
        assert!(
            sigs.iter().any(|s| s.params.len() == 3),
            "http_get should take 3 args"
        );
    }

    #[test]
    fn http_request_sig_param_shape() {
        let sig = http_request_sig();
        assert_eq!(
            sig.params,
            vec![
                TySpec::Bytes,
                TySpec::Bytes,
                TySpec::Bytes,
                TySpec::Bytes,
                TySpec::Binary(WidthSpec(64)),
            ]
        );
    }
}
