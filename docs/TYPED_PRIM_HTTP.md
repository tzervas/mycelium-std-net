# `install_typed_http_prims` — contract (S-STD-NET-SAFE-HTTP, PKG-LINKAGE)

**Hub:** https://github.com/tzervas/mycelium-lang/issues/44
**Frozen surface:** `docs/planning/orchestration/surfaces/S-STD-NET-SAFE-HTTP.md` in
`tzervas/mycelium-lang`.

## Why this exists next to `wild:http_request`

`wild:http_request` (see [`INSTALL_HTTP_HOST_OPS.md`](INSTALL_HTTP_HOST_OPS.md)) works — a live
HTTPS 200 from a public host via ureq+rustls, verified — but it goes through `wild`'s
ascription-on-faith boundary: a `.myc` `fn` may declare any result type it likes for a `wild` call,
and both `myc check` and `myc run` report success even when that ascription does not match what
the op actually returns. Exactly the wrong property for an HTTP client.

This crate's `typed-prims` feature adds a second, disjoint `prim:` door whose signature — arity,
per-argument type, result type, and declared effects — is registered as a real
`mycelium_interp::typed::PrimSig` that a checker (`mycelium-l1`, a later PKG-LINKAGE lane) can
verify a call site against, instead of trusting an ascription.

`wild:http_request` / `install_http_host_ops` are byte-for-byte unchanged by this feature.

## Two features, two purposes

| Feature | Needs | Gives |
|---|---|---|
| `typed-prims` | `mycelium-interp`, `mycelium-core` (no `ureq`/`rustls`) | `typed_prim_sigs() -> Vec<PrimSig>` — pure signature data for a checker |
| `typed-prims` + `client` | + `ureq`/`rustls` | `install_typed_http_prims(&mut TypedPrimRegistry)` — the real dispatch |

Measured: `cargo tree -e normal --no-default-features --features typed-prims` carries no
`ureq`/`rustls` edge — a static `myc check` build can load the signatures without linking a TLS
stack.

## Signature (`features = "typed-prims", "client"`)

```rust,ignore
use mycelium_interp::typed::TypedPrimRegistry;
use mycelium_std_net::install_typed_http_prims;

let mut reg = TypedPrimRegistry::empty();
install_typed_http_prims(&mut reg);
// registers prim:http_request, prim:http_get
```

## v0 ops (blocking-hypha; same I/O model as `wild:http_request`)

| `prim:` name | Arity / result | Effects |
|---|---|---|
| `http_request` | `(method, url, headers, body, timeout_ms) -> Adt("std.net.HttpResponse")` | `["net"]` |
| `http_get` | `(url, headers, timeout_ms) -> Adt("std.net.HttpResponse")` | `["net"]` |

### Arg encoding (identical to `wild:http_request`'s, see `INSTALL_HTTP_HOST_OPS.md`)

| Role | `TySpec` | Notes |
|---|---|---|
| method (http_request only) | `Bytes` | UTF-8 method (`GET`, `POST`, …) |
| url | `Bytes` | UTF-8 absolute URL (`http://` or `https://`) |
| headers | `Bytes` | `name\tvalue\n` lines; empty = none |
| body (http_request only) | `Bytes` | request body (may be empty) |
| timeout_ms | `Binary{64}` | unsigned magnitude; `0` = no explicit timeout |

### Result: `Adt("std.net.HttpResponse")`

The checked *type* is `Adt("std.net.HttpResponse")` — this is what a caller's declared result
type is verified against at `myc check` time, replacing `wild:http_request`'s opaque,
ascription-trusted `Seq<Bytes>{3}`. The runtime encoding underneath is intentionally kept
byte-identical to `wild:http_request`'s own triple (status/headers/body — see
`INSTALL_HTTP_HOST_OPS.md`'s result table): this surface adds a checked type, not a second wire
format for the same bytes.

Ambient network results are **`Declared`** (VR-5), exactly like `wild:http_request`. Transport /
encode failures are explicit `EvalError::PrimType` (G2 never-silent) — never a silent empty body.

## Effect: `"net"`, not `"ffi"`

Both `PrimSig`s declare `effects: vec!["net".into()]`. A `.myc` caller must cover `!{net}`
specifically; a caller declaring `!{ffi}` instead is refused (the effect name comes from the
registered signature, not a hardcoded constant).

## Who calls

A checker (`mycelium-l1`) resolves `use`-imported typed-prim call sites against
`typed_prim_sigs()`. `myc` (or an embedder) installs the real dispatch via
`install_typed_http_prims` before host-capable evaluation, the same way it installs
`install_http_host_ops` for the `wild` path today.
