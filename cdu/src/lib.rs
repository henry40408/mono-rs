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

mod cdu;
mod error;
mod opts;

pub use crate::cdu::Cdu;
pub use crate::error::PublicIPError;
pub use crate::opts::Opts;
