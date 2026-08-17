use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Manager,
    Kitchen,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Manager => "manager",
            Role::Kitchen => "kitchen",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "manager" => Some(Role::Manager),
            "kitchen" => Some(Role::Kitchen),
            _ => None,
        }
    }

    pub fn can_edit_recipes(self) -> bool {
        matches!(self, Role::Manager)
    }

    pub fn can_see_costs(self) -> bool {
        matches!(self, Role::Manager)
    }

    pub fn filter_cost<T>(self, value: Option<T>) -> Option<T> {
        if self.can_see_costs() {
            value
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_manager_sees_costs() {
        assert!(Role::Manager.can_see_costs());
        assert!(!Role::Kitchen.can_see_costs());
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: Role,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Public user fields for API responses (no password hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: Role,
}

impl From<&User> for UserPublic {
    fn from(u: &User) -> Self {
        Self {
            id: u.id,
            email: u.email.clone(),
            name: u.name.clone(),
            role: u.role,
        }
    }
}
