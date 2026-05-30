use actix_web::{dev::ServiceRequest, Error, HttpMessage};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

use crate::models::Claims;

pub async fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    println!("DEBUG JWT_SECRET: {}", secret);
    println!("DEBUG token: {}", credentials.token());

    let result = decode::<Claims>(
        credentials.token(),
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    );

    match result {
        Ok(token_data) => {
            req.extensions_mut().insert(token_data.claims);
            Ok(req)
        }
        Err(e) => {
            println!("DEBUG decode error: {:?}", e);
            let error = actix_web::error::ErrorUnauthorized(
                serde_json::json!({"error": "Invalid or expired token"}).to_string(),
            );
            Err((error, req))
        }
    }
}