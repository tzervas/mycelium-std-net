# install_http_host_ops — contract (WP-6 / S-STD-NET)

## Signature (`feature = "host-registry"`)

```rust
use mycelium_interp::PrimRegistry;
use mycelium_std_net::install_http_host_ops;

let mut reg = PrimRegistry::with_builtins();
install_http_host_ops(&mut reg);
```

## v0 op (blocking-hypha)

| wild name | Arity / result | Encoding |
|-----------|----------------|----------|
| `http_request` | `(method, url, headers, body, timeout_ms) → Seq<Bytes>{3}` | see below |

### Arg encoding

| # | Name | Repr | Notes |
|---|------|------|-------|
| 0 | method | `Bytes` | UTF-8 method (`GET`, `POST`, …); empty → `PrimType` |
| 1 | url | `Bytes` | UTF-8 absolute URL (`http://` or `https://`) |
| 2 | headers | `Bytes` | line format `name\tvalue\n` (tab-separated; empty = no headers) |
| 3 | body | `Bytes` | request body (may be empty) |
| 4 | timeout_ms | `Binary{W}` | unsigned magnitude; `0` = no explicit timeout |

### Result encoding

`Seq { elem: Bytes, len: 3 }`:

| index | content |
|-------|---------|
| 0 | status as 2-byte big-endian `u16` |
| 1 | response headers, same `name\tvalue\n` format |
| 2 | response body bytes |

Ambient network results are **`Declared`** (VR-5). Transport / encode failures are
explicit [`EvalError::PrimType`] (G2 never-silent) — never a silent empty body.

## Who calls

`myc` CLI (or embedder) after floor host ops, before host-capable evaluation. CLI
may call both `std-sys-host::install_default_host_ops` and
`std-net::install_http_host_ops` on the same registry.
