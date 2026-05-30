use serde::{Deserialize, Serialize};

/// JWT claims shared across auth routes and middleware.
/// `sub` stores the username; `exp` is the Unix expiry timestamp.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // username
    pub exp: usize,  // expiry timestamp
}
