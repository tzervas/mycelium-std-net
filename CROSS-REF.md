# CROSS-REF — mycelium-std-net

Mycelium-internal dependencies only (steer handoff §6.1; external crates stay in Cargo
metadata). Pinned revs are the fixed (buildable) tips for WP-6 co-dev with the host-registry train.

| Interface consumed | Repo | Pinned rev | Notes |
|---|---|---|---|
| mycelium-interp (feature `host-registry`) | https://github.com/tzervas/mycelium-runtime | `b11fdfe8f33843b68ea6bc86f48f380532ca86c3` | train/gap-closure-host-call-registry tip (`register_host` / `install_host_ops`) |
| mycelium-core (feature `host-registry`) | https://github.com/tzervas/mycelium-core | `46d2515cbd86d2ae4d1365f4adcd2796737e9f0b` | same core rev as interp on that tip |

**External (crates.io):** `ureq` 3.3.0 (default features: rustls + gzip).

**Owning surface:** monorepo `docs/planning/orchestration/surfaces/S-STD-NET.md` · hub #30 WP-6.
**Stack decision:** spike S3 (2026-08-01) — ureq + rustls, blocking, client-only.
