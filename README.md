# mycelium-std-net

<!-- FLEET-BADGES:BEGIN -->
[![CI](https://github.com/tzervas/mycelium-std-net/actions/workflows/fleet-ci.yml/badge.svg?branch=main)](https://github.com/tzervas/mycelium-std-net/actions/workflows/fleet-ci.yml?query=branch%3Amain)
[![Security](https://github.com/tzervas/mycelium-std-net/actions/workflows/fleet-security.yml/badge.svg?branch=main)](https://github.com/tzervas/mycelium-std-net/actions/workflows/fleet-security.yml?query=branch%3Amain)
[![Runner](https://img.shields.io/badge/runs--on-self--hosted%20podman-informational)](https://github.com/tzervas/gha-runner-ctl)
<!-- FLEET-BADGES:END -->

Rust std.net phylum: **blocking HTTPS client** (ureq + rustls) for the Mycelium train.

| Field | Value |
|---|---|
| **Work package** | WP-6 / S-STD-NET; WP-10 / PKG-LINKAGE (S-STD-NET-SAFE-HTTP) |
| **Hub** | [mycelium-lang#30](https://github.com/tzervas/mycelium-lang/issues/30), [mycelium-lang#44](https://github.com/tzervas/mycelium-lang/issues/44) |
| **Stack** | ureq 3.x + rustls (blocking; client-only), feature `client` |
| **Wild** | `wild:http_request` — ascription-trusted (feature `host-registry`) |
| **Typed** | `prim:http_request` / `prim:http_get` — checked (feature `typed-prims`) |
| **License** | MIT |
| **Honesty** | Guarantee tags stay **Declared** until differential upgrades |
| **`default` features** | `[]` — every feature above is opt-in (BEHAVIOR NOTE: before the S-STD-NET-SAFE-HTTP split, `ureq` was a non-optional dependency, so the plain-Rust `http_request` client was always compiled in regardless of flags even though `default` was already `[]`; it now genuinely needs `--features client`) |

## Pin-home FREEZE

**Decision: new pin** `tzervas/mycelium-std-net` (not a module under `std-io`). Surface default; avoids std-io bloat for process/codec work.

## API (Rust)

Needs feature `client` (see [Build](#build) — split from `typed-prims` so a checker never needs
to link ureq/rustls just to read a signature, S-STD-NET-SAFE-HTTP / PKG-LINKAGE):

```rust,no_run
# #[cfg(feature = "client")] fn __doctest() -> Result<(), mycelium_std_net::NetError> {
use mycelium_std_net::{http_request, HttpResponse};

let resp: HttpResponse = http_request(
    "GET",
    "https://example.com/",
    &[("Accept".into(), "text/plain".into())],
    b"",
    Some(5_000), // timeout_ms
)?;
assert!(resp.status >= 100);
# Ok(())
# }
```

`POST` (and other methods) use the same entry point with a body slice.

## Checked, non-`wild` API (feature `typed-prims`; S-STD-NET-SAFE-HTTP)

```rust,ignore
// requires feature = "typed-prims" (pure signature data — no ureq/rustls edge)
use mycelium_std_net::typed_prim_sigs;

for sig in typed_prim_sigs() {
    assert_eq!(sig.effects, vec!["net".to_owned()]);
}
```

```rust,ignore
// requires feature = "typed-prims" AND "client" for the actual dispatch
use mycelium_interp::typed::TypedPrimRegistry;
use mycelium_std_net::install_typed_http_prims;

let mut reg = TypedPrimRegistry::empty();
install_typed_http_prims(&mut reg);
// registers prim:http_request, prim:http_get
```

## Wild host install (feature `host-registry`)

```rust,ignore
// requires feature = "host-registry"
use mycelium_interp::PrimRegistry;
use mycelium_std_net::install_http_host_ops;

let mut reg = PrimRegistry::with_builtins();
install_http_host_ops(&mut reg);
// registers wild:http_request
```

## Non-goals (v0)

- Server listen / accept
- HTTP/2 multiplexing requirements
- Async / tokio

## Build

MSRV 1.96.1.

```bash
cargo test                                    # error.rs, guarantee_matrix.rs only (default = [])
cargo test --features client                  # + src/client.rs's http_request
cargo test --features host-registry           # + wild:http_request (implies client)
cargo test --no-default-features --features typed-prims          # pure PrimSig data, no ureq/rustls
cargo test --features typed-prims,host-registry                  # + prim:http_request/http_get dispatch
cargo tree -e normal --no-default-features --features typed-prims  # confirm no ureq/rustls edge
```

Live HTTPS smoke (both `wild:http_request` and the typed `prim:` differential check) is
`#[ignore]` (needs outbound network). Run it with every feature enabled, so every doctest example
also has the symbols its own snippet needs in scope:

```bash
cargo test --features typed-prims,host-registry -- --ignored
```

## Consumers

`gha-runner-ctl`, `tg-agent-relay` (train ports WP-7 / WP-8).
