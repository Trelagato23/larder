use anyhow::Result;
use sqlx::SqlitePool;

pub struct UserService {
    _pool: SqlitePool,
}

impl UserService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { _pool: pool }
    }

    pub async fn create_user(
        &self,
        _email: &str,
        _name: &str,
        _password: &str,
    ) -> Result<uuid::Uuid> {
        anyhow::bail!("User management is not implemented yet")
    }

    pub async fn authenticate(
        &self,
        _email: &str,
        _password: &str,
    ) -> Result<Option<crate::models::User>> {
        anyhow::bail!("User authentication is not implemented yet")
    }
}
