#[derive(Debug, Clone)]
pub struct PublicIPError;

impl std::fmt::Display for PublicIPError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to determine public IPv4 address")
    }
}

impl std::error::Error for PublicIPError {}
