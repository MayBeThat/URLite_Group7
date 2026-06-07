use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Clone)]
pub struct BaseUrl(pub String);

#[derive(Clone)]
pub struct JwtSecret(pub String);

#[derive(Clone)]
pub struct FrontendDir(pub String);
