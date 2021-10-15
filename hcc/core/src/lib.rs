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

//! HTTPS Certificate Check

pub use check_client::CheckClient;
pub use check_result::{CheckResult, CheckResultJSON, CheckState};

mod check_client;
mod check_result;
