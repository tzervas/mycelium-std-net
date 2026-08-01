# std.net — blocking HTTPS client (v0 sketch)

**Status:** implement (WP-6). Spec slice; full monorepo docs remain authority.

## Ops

| op | effect | guarantee |
|----|--------|-----------|
| `http_request` | ambient network I/O (may block) | Declared |

## Non-goals v0

Server listen, HTTP/2 mux, async.
