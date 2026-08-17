use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::models::{NoteAuthorRole, NoteSeverity, RecipeNote};

#[derive(FromRow)]
struct NoteRow {
    id: String,
    recipe_id: String,
    body: String,
    severity: String,
    author_role: String,
    author_name: String,
    author_user_id: Option<String>,
    signature: Option<String>,
    signed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<NoteRow> for RecipeNote {
    fn from(r: NoteRow) -> Self {
        Self {
            id: Uuid::parse_str(&r.id).unwrap_or_default(),
            recipe_id: Uuid::parse_str(&r.recipe_id).unwrap_or_default(),
            body: r.body,
            severity: NoteSeverity::parse(&r.severity).unwrap_or(NoteSeverity::Subtle),
            author_role: NoteAuthorRole::parse(&r.author_role).unwrap_or(NoteAuthorRole::Team),
            author_name: r.author_name,
            author_user_id: r
                .author_user_id
                .as_deref()
                .and_then(|s| Uuid::parse_str(s).ok()),
            signature: r.signature,
            signed_at: r.signed_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Clone)]
pub struct RecipeNoteService {
    pool: SqlitePool,
}

impl RecipeNoteService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_for_recipe(&self, recipe_id: Uuid) -> Result<Vec<RecipeNote>> {
        let rows: Vec<NoteRow> = sqlx::query_as(
            r#"
            SELECT id, recipe_id, body, severity, author_role, author_name,
                   author_user_id, signature, signed_at, created_at, updated_at
            FROM recipe_notes
            WHERE recipe_id = ?
            ORDER BY
                CASE severity WHEN 'flagged' THEN 0 ELSE 1 END,
                created_at DESC
            "#,
        )
        .bind(recipe_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(RecipeNote::from).collect())
    }

    pub async fn get(&self, note_id: Uuid) -> Result<Option<RecipeNote>> {
        let row: Option<NoteRow> = sqlx::query_as(
            r#"
            SELECT id, recipe_id, body, severity, author_role, author_name,
                   author_user_id, signature, signed_at, created_at, updated_at
            FROM recipe_notes WHERE id = ?
            "#,
        )
        .bind(note_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(RecipeNote::from))
    }

    pub async fn create(
        &self,
        recipe_id: Uuid,
        body: &str,
        severity: NoteSeverity,
        author_role: NoteAuthorRole,
        author_name: &str,
        author_user_id: Option<Uuid>,
    ) -> Result<RecipeNote> {
        let body = body.trim();
        if body.is_empty() {
            bail!("note body required");
        }
        let name = author_name.trim();
        if name.is_empty() {
            bail!("author name required");
        }
        let id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO recipe_notes
              (id, recipe_id, body, severity, author_role, author_name, author_user_id,
               signature, signed_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(recipe_id.to_string())
        .bind(body)
        .bind(severity.as_str())
        .bind(author_role.as_str())
        .bind(name)
        .bind(author_user_id.map(|u| u.to_string()))
        .bind(name)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.get(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("note insert missing"))
    }

    pub async fn update_body_severity(
        &self,
        note_id: Uuid,
        body: Option<&str>,
        severity: Option<NoteSeverity>,
    ) -> Result<RecipeNote> {
        let existing = self
            .get(note_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("note not found"))?;
        let body = body
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(existing.body.as_str());
        let severity = severity.unwrap_or(existing.severity);
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE recipe_notes
            SET body = ?, severity = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(body)
        .bind(severity.as_str())
        .bind(now)
        .bind(note_id.to_string())
        .execute(&self.pool)
        .await?;
        self.get(note_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("note missing after update"))
    }

    /// Manager sign-off: typed signature name + timestamp.
    pub async fn sign(&self, note_id: Uuid, signature: &str) -> Result<RecipeNote> {
        let sig = signature.trim();
        if sig.is_empty() {
            bail!("signature required");
        }
        let now = Utc::now();
        let n = sqlx::query(
            r#"
            UPDATE recipe_notes
            SET signature = ?, signed_at = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(sig)
        .bind(now)
        .bind(now)
        .bind(note_id.to_string())
        .execute(&self.pool)
        .await?
        .rows_affected();
        if n == 0 {
            bail!("note not found");
        }
        self.get(note_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("note missing after sign"))
    }

    pub async fn delete(&self, note_id: Uuid) -> Result<bool> {
        let n = sqlx::query("DELETE FROM recipe_notes WHERE id = ?")
            .bind(note_id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(n > 0)
    }

    /// Recipe ids that currently have at least one flagged note.
    pub async fn flagged_recipe_ids(&self) -> Result<Vec<Uuid>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT recipe_id FROM recipe_notes WHERE severity = 'flagged'
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(id,)| Uuid::parse_str(&id).ok())
            .collect())
    }

    /// Recipe notes + board posts, flagged first, then newest.
    pub async fn list_feed(&self) -> Result<Vec<FeedPost>> {
        let rows: Vec<FeedRow> = sqlx::query_as(
            r#"
            SELECT * FROM (
                SELECT n.id,
                       'recipe' AS kind,
                       n.recipe_id,
                       r.name AS recipe_name,
                       n.body, n.severity, n.author_role, n.author_name,
                       n.author_user_id, n.signature, n.signed_at,
                       n.created_at, n.updated_at
                FROM recipe_notes n
                LEFT JOIN recipes r ON r.id = n.recipe_id
                UNION ALL
                SELECT p.id,
                       'board' AS kind,
                       NULL AS recipe_id,
                       NULL AS recipe_name,
                       p.body, p.severity, p.author_role, p.author_name,
                       p.author_user_id, p.signature, p.signed_at,
                       p.created_at, p.updated_at
                FROM board_posts p
            )
            ORDER BY
                CASE severity WHEN 'flagged' THEN 0 ELSE 1 END,
                created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(FeedPost::from).collect())
    }

    pub async fn get_board(&self, id: Uuid) -> Result<Option<FeedPost>> {
        let row: Option<FeedRow> = sqlx::query_as(
            r#"
            SELECT id, 'board' AS kind, NULL AS recipe_id, NULL AS recipe_name,
                   body, severity, author_role, author_name, author_user_id,
                   signature, signed_at, created_at, updated_at
            FROM board_posts WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(FeedPost::from))
    }

    pub async fn create_board(
        &self,
        body: &str,
        severity: NoteSeverity,
        author_role: NoteAuthorRole,
        signature: &str,
        author_user_id: Option<Uuid>,
    ) -> Result<FeedPost> {
        let body = body.trim();
        if body.is_empty() {
            bail!("note body required");
        }
        let name = signature.trim();
        if name.is_empty() {
            bail!("signature required");
        }
        let id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO board_posts
              (id, body, severity, author_role, author_name, author_user_id,
               signature, signed_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(body)
        .bind(severity.as_str())
        .bind(author_role.as_str())
        .bind(name)
        .bind(author_user_id.map(|u| u.to_string()))
        .bind(name)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_board(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("board post insert missing"))
    }

    pub async fn update_board(
        &self,
        id: Uuid,
        body: Option<&str>,
        severity: Option<NoteSeverity>,
    ) -> Result<FeedPost> {
        let existing = self
            .get_board(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("post not found"))?;
        let body = body
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(existing.body.as_str());
        let severity = severity.unwrap_or(existing.severity);
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE board_posts
            SET body = ?, severity = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(body)
        .bind(severity.as_str())
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        self.get_board(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("post missing after update"))
    }

    pub async fn sign_board(&self, id: Uuid, signature: &str) -> Result<FeedPost> {
        let sig = signature.trim();
        if sig.is_empty() {
            bail!("signature required");
        }
        let now = Utc::now();
        let n = sqlx::query(
            r#"
            UPDATE board_posts
            SET signature = ?, signed_at = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(sig)
        .bind(now)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?
        .rows_affected();
        if n == 0 {
            bail!("post not found");
        }
        self.get_board(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("post missing after sign"))
    }

    pub async fn delete_board(&self, id: Uuid) -> Result<bool> {
        let n = sqlx::query("DELETE FROM board_posts WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(n > 0)
    }
}

#[derive(Debug, Clone)]
pub struct FeedPost {
    pub id: Uuid,
    pub kind: String,
    pub recipe_id: Option<Uuid>,
    pub recipe_name: Option<String>,
    pub body: String,
    pub severity: NoteSeverity,
    pub author_role: NoteAuthorRole,
    pub author_name: String,
    pub author_user_id: Option<Uuid>,
    pub signature: Option<String>,
    pub signed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct FeedRow {
    id: String,
    kind: String,
    recipe_id: Option<String>,
    recipe_name: Option<String>,
    body: String,
    severity: String,
    author_role: String,
    author_name: String,
    author_user_id: Option<String>,
    signature: Option<String>,
    signed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<FeedRow> for FeedPost {
    fn from(r: FeedRow) -> Self {
        Self {
            id: Uuid::parse_str(&r.id).unwrap_or_default(),
            kind: if r.kind == "recipe" {
                "recipe".into()
            } else {
                "board".into()
            },
            recipe_id: r.recipe_id.as_deref().and_then(|s| Uuid::parse_str(s).ok()),
            recipe_name: r.recipe_name.filter(|s| !s.is_empty()),
            body: r.body,
            severity: NoteSeverity::parse(&r.severity).unwrap_or(NoteSeverity::Subtle),
            author_role: NoteAuthorRole::parse(&r.author_role).unwrap_or(NoteAuthorRole::Team),
            author_name: r.author_name,
            author_user_id: r
                .author_user_id
                .as_deref()
                .and_then(|s| Uuid::parse_str(s).ok()),
            signature: r.signature,
            signed_at: r.signed_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}
