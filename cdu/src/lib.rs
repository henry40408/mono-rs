#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

//! Cloudflare DNS record update

use thiserror::Error;

pub use crate::cdu::Cdu;

mod cdu;

/// Error from [`Cdu`]
#[derive(Clone, Copy, Debug, Error)]
pub enum RecoverableError {
    /// Recoverable: Failed to determine IPv4 address
    #[error("failed to determine IPv4 address")]
    IpV4,
}
