use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header, request::Parts},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use larder_core::models::{Role, User, UserPublic};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

const DEFAULT_JWT_SECRET: &str = "larder-dev-secret-change-me";
const TOKEN_TTL_HOURS: i64 = 12;

#[derive(Clone)]
pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JwtKeys {
    pub fn from_env() -> Self {
        let secret = std::env::var("LARDER_JWT_SECRET").unwrap_or_else(|_| DEFAULT_JWT_SECRET.into());
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    pub fn issue(&self, user: &User) -> Result<String, jsonwebtoken::errors::Error> {
        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            name: user.name.clone(),
            role: user.role.as_str().to_string(),
            exp: (Utc::now() + Duration::hours(TOKEN_TTL_HOURS)).timestamp() as usize,
        };
        encode(&Header::default(), &claims, &self.encoding)
    }

    pub fn decode(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let data = decode::<Claims>(token, &self.decoding, &Validation::default())?;
        Ok(data.claims)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub exp: usize,
}

/// Authenticated user extracted from Bearer JWT.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: Role,
}

impl AuthUser {
    pub fn public(&self) -> UserPublic {
        UserPublic {
            id: self.id,
            email: self.email.clone(),
            name: self.name.clone(),
            role: self.role,
        }
    }

    pub fn from_claims(claims: &Claims) -> Result<Self, (StatusCode, String)> {
        let id = Uuid::parse_str(&claims.sub)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token subject".into()))?;
        let role = Role::parse(&claims.role)
            .ok_or((StatusCode::UNAUTHORIZED, "invalid role in token".into()))?;
        Ok(Self {
            id,
            email: claims.email.clone(),
            name: claims.name.clone(),
            role,
        })
    }
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "login required".into(),
            ))?;

        let token = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "expected Bearer token".into(),
            ))?;

        let claims = state
            .jwt
            .decode(token)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid or expired token".into()))?;

        AuthUser::from_claims(&claims)
    }
}

/// Optional store location from `X-Larder-Location` header (validated when present).
#[derive(Debug, Clone)]
pub struct LocationContext(pub Option<Uuid>);

impl LocationContext {
    pub async fn validate(
        &self,
        user: &AuthUser,
        state: &Arc<AppState>,
    ) -> Result<(), (StatusCode, String)> {
        if let Some(loc_id) = self.0 {
            let ok = state
                .locations
                .user_can_access(user.id, loc_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            if !ok {
                return Err((StatusCode::FORBIDDEN, "location not allowed".into()));
            }
        }
        Ok(())
    }
}

impl FromRequestParts<Arc<AppState>> for LocationContext {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let id = parts
            .headers
            .get("X-Larder-Location")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s.trim()).ok());
        Ok(LocationContext(id))
    }
}

/// Manager-only access (recipe edit/delete, import, tags).
#[allow(dead_code)]
pub struct ManagerUser(pub AuthUser);

impl FromRequestParts<Arc<AppState>> for ManagerUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !user.role.can_edit_recipes() {
            return Err((
                StatusCode::FORBIDDEN,
                "manager role required".into(),
            ));
        }
        Ok(ManagerUser(user))
    }
}
