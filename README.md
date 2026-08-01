# mycelium-std-net

<!-- FLEET-BADGES:BEGIN -->
[![CI](https://github.com/tzervas/mycelium-std-net/actions/workflows/fleet-ci.yml/badge.svg?branch=main)](https://github.com/tzervas/mycelium-std-net/actions/workflows/fleet-ci.yml?query=branch%3Amain)
[![Security](https://github.com/tzervas/mycelium-std-net/actions/workflows/fleet-security.yml/badge.svg?branch=main)](https://github.com/tzervas/mycelium-std-net/actions/workflows/fleet-security.yml?query=branch%3Amain)
[![Runner](https://img.shields.io/badge/runs--on-self--hosted%20podman-informational)](https://github.com/tzervas/gha-runner-ctl)
<!-- FLEET-BADGES:END -->

Rust std.net phylum: **blocking HTTPS client** (ureq + rustls) for the Mycelium train.

| Field | Value |
|---|---|
| **Work package** | WP-6 / S-STD-NET |
| **Hub** | [mycelium-lang#30](https://github.com/tzervas/mycelium-lang/issues/30) |
| **Stack** | ureq 3.x + rustls (blocking; client-only) |
| **Wild** | `wild:http_request` (feature `host-registry`) |
| **License** | MIT |
| **Honesty** | Guarantee tags stay **Declared** until differential upgrades |

## Pin-home FREEZE

**Decision: new pin** `tzervas/mycelium-std-net` (not a module under `std-io`). Surface default; avoids std-io bloat for process/codec work.

## API (Rust)

```rust,no_run
use mycelium_std_net::{http_request, HttpResponse, NetError};

fn main() -> Result<(), NetError> {
    let resp: HttpResponse = http_request(
        "GET",
        "https://example.com/",
        &[("Accept".into(), "text/plain".into())],
        b"",
        Some(5_000), // timeout_ms
    )?;
    assert!(resp.status >= 100);
    Ok(())
}
```

`POST` (and other methods) use the same entry point with a body slice.

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
cargo test
cargo test --features host-registry
```

Live HTTPS smoke is `#[ignore]` (needs network):

```bash
cargo test -- --ignored
```

## Consumers

`gha-runner-ctl`, `tg-agent-relay` (train ports WP-7 / WP-8).
