// nodule: mycelium-std-net — blocking HTTPS client (WP-6 / S-STD-NET; spike S3)
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

//! # Purpose
//!
//! Client-only, blocking HTTP(S) for Mycelium first ports (`gha-runner-ctl`,
//! `tg-agent-relay`). Stack is **ureq + rustls** (spike S3, 2026-08-01).
//!
//! # Honesty
//!
//! Every network op is **`Declared`** (RFC-0016 §4.1 C2 / VR-5): ambient OS/TLS/DNS
//! with no checked bound. Failures are explicit [`NetError`] / [`EvalError::PrimType`]
//! (G2 never-silent).
//!
//! # Modules
//!
//! - [`client`] — pure Rust `http_request` surface over ureq. Feature `client`.
//! - [`error`] — never-silent error carrier.
//! - [`guarantee_matrix`] — op × guarantee table.
//! - [`host_registry`] — `wild:http_request` install (feature `host-registry`).
//! - [`typed_prims`] — checked, non-`wild` `prim:http_request`/`prim:http_get`
//!   (S-STD-NET-SAFE-HTTP, feature `typed-prims`).

#[cfg(feature = "client")]
pub mod client;
pub mod error;
pub mod guarantee_matrix;

/// `wild:http_request` install (`install_http_host_ops`). Feature `host-registry`.
#[cfg(feature = "host-registry")]
pub mod host_registry;

/// `prim:http_request` / `prim:http_get` — checked, non-`wild` HTTP (S-STD-NET-SAFE-HTTP,
/// PKG-LINKAGE). Feature `typed-prims`.
#[cfg(feature = "typed-prims")]
pub mod typed_prims;

#[cfg(feature = "client")]
pub use client::{http_request, HttpResponse};
pub use error::NetError;
pub use guarantee_matrix::{GuaranteeRow, GUARANTEE_MATRIX};

#[cfg(feature = "host-registry")]
pub use host_registry::install_http_host_ops;

#[cfg(feature = "typed-prims")]
pub use typed_prims::{typed_prim_sigs, HTTP_RESPONSE_ADT};

#[cfg(all(feature = "typed-prims", feature = "client"))]
pub use typed_prims::install_typed_http_prims;

#[cfg(all(test, feature = "client"))]
mod tests;
