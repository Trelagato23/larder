use anyhow::{Context, Result};
use argon2::{Argon2, password_hash::PasswordHasher};
use password_hash::{PasswordHash, PasswordVerifier, SaltString, rand_core::OsRng};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::{Role, User};

pub struct UserService {
    pool: SqlitePool,
}

impl UserService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn hash_password(password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("hash password: {e}"))?
            .to_string();
        Ok(hash)
    }

    pub fn verify_password(password: &str, password_hash: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(password_hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }

    pub async fn create_user(
        &self,
        email: &str,
        name: &str,
        password: &str,
        role: Role,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let password_hash = Self::hash_password(password)?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO users (id, email, name, password_hash, role, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(email.trim().to_ascii_lowercase())
        .bind(name)
        .bind(password_hash)
        .bind(role.as_str())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .context("insert user")?;
        Ok(id)
    }

    /// Insert user with fixed id if missing (seed / default).
    pub async fn ensure_user(
        &self,
        id: Uuid,
        email: &str,
        name: &str,
        password: &str,
        role: Role,
    ) -> Result<()> {
        let existing = self.get_by_id(id).await?;
        if existing.is_some() {
            // Upgrade legacy "!" placeholder hashes so demo login works.
            if let Some(u) = existing {
                if u.password_hash == "!" || !Self::verify_password(password, &u.password_hash) {
                    // Only reset if hash is placeholder
                    if u.password_hash == "!" {
                        let password_hash = Self::hash_password(password)?;
                        sqlx::query(
                            "UPDATE users SET password_hash = ?, role = ?, name = ?, email = ?, updated_at = ? WHERE id = ?",
                        )
                        .bind(password_hash)
                        .bind(role.as_str())
                        .bind(name)
                        .bind(email.trim().to_ascii_lowercase())
                        .bind(Utc::now().to_rfc3339())
                        .bind(id.to_string())
                        .execute(&self.pool)
                        .await?;
                    } else {
                        // Ensure role column is set for existing users
                        sqlx::query("UPDATE users SET role = COALESCE(NULLIF(role, ''), ?) WHERE id = ?")
                            .bind(role.as_str())
                            .bind(id.to_string())
                            .execute(&self.pool)
                            .await
                            .ok();
                    }
                }
            }
            return Ok(());
        }

        let password_hash = Self::hash_password(password)?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO users (id, email, name, password_hash, role, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(email.trim().to_ascii_lowercase())
        .bind(name)
        .bind(password_hash)
        .bind(role.as_str())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .context("ensure user")?;
        Ok(())
    }

    pub async fn authenticate(&self, email: &str, password: &str) -> Result<Option<User>> {
        let user = self.get_by_email(email).await?;
        let Some(user) = user else {
            return Ok(None);
        };
        if !Self::verify_password(password, &user.password_hash) {
            return Ok(None);
        }
        Ok(Some(user))
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, name, password_hash, role, avatar_url, created_at, updated_at FROM users WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .context("get user by id")?;
        Ok(row.map(UserRow::into_user).transpose()?)
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<User>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, name, password_hash, role, avatar_url, created_at, updated_at FROM users WHERE email = ?",
        )
        .bind(email.trim().to_ascii_lowercase())
        .fetch_optional(&self.pool)
        .await
        .context("get user by email")?;
        Ok(row.map(UserRow::into_user).transpose()?)
    }
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    email: String,
    name: String,
    password_hash: String,
    role: Option<String>,
    avatar_url: Option<String>,
    created_at: String,
    updated_at: String,
}

impl UserRow {
    fn into_user(self) -> Result<User> {
        let role = self
            .role
            .as_deref()
            .and_then(Role::parse)
            .unwrap_or(Role::Manager);
        Ok(User {
            id: Uuid::parse_str(&self.id).context("user id")?,
            email: self.email,
            name: self.name,
            password_hash: self.password_hash,
            role,
            avatar_url: self.avatar_url,
            created_at: parse_dt(&self.created_at)?,
            updated_at: parse_dt(&self.updated_at)?,
        })
    }
}

fn parse_dt(s: &str) -> Result<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    // SQLite datetime('now') style
    let naive = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
        .with_context(|| format!("parse datetime: {s}"))?;
    Ok(DateTime::from_naive_utc_and_offset(naive, Utc))
}
