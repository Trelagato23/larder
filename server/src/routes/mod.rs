use axum::{
    Router,
    http::StatusCode,
    routing::{get, post},
};
use std::sync::Arc;
use tower_http::services::ServeDir;

use crate::AppState;

pub mod auth;
pub mod cookbooks;
pub mod export;
pub mod health;
pub mod import;
pub mod production;
pub mod locations;
pub mod ingredients;
pub mod meal_plans;
pub mod prep_sheet;
pub mod posts;
pub mod recipes;
pub mod shopping;
pub mod tags;

pub fn create_router(state: Arc<AppState>) -> Router {
    let static_dir = std::env::var("LARDER_STATIC_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/static").to_string()
    });
    let index_path = format!("{static_dir}/index.html");

    Router::new()
        .route("/", get({
            let index_path = index_path.clone();
            move || serve_index(index_path.clone())
        }))
        .route("/health", get(health::handler))
        // Auth (public login)
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/locations", get(locations::list))
        .route("/api/production", get(production::list).post(production::create))
        .route(
            "/api/production/{id}/items",
            get(production::items).post(production::add_item),
        )
        .route(
            "/api/production/{plan_id}/items/{item_id}",
            axum::routing::delete(production::remove_item),
        )
        .route(
            "/api/production/{id}/pull-list",
            get(production::pull_list_json),
        )
        .route(
            "/api/production/{id}/pull-list/print",
            get(production::pull_list_html),
        )
        // Recipes — reads: any logged-in user; writes: manager only
        .route("/api/recipes", get(recipes::list).post(recipes::create))
        .route("/api/recipes/search", get(recipes::search))
        .route(
            "/api/recipes/{id}",
            get(recipes::show)
                .put(recipes::update)
                .delete(recipes::delete),
        )
        .route("/api/recipes/{id}/ingredients", get(recipes::ingredients))
        .route("/api/recipes/{id}/steps", get(recipes::steps))
        .route("/api/recipes/{id}/related", get(recipes::related))
        .route(
            "/api/recipes/{id}/notes",
            get(recipes::list_notes).post(recipes::create_note),
        )
        .route(
            "/api/recipes/{id}/notes/{note_id}",
            axum::routing::put(recipes::update_note).delete(recipes::delete_note),
        )
        .route(
            "/api/recipes/{id}/notes/{note_id}/sign",
            post(recipes::sign_note),
        )
        .route("/api/posts", get(posts::list).post(posts::create))
        .route(
            "/api/posts/{id}",
            axum::routing::put(posts::update).delete(posts::delete),
        )
        .route("/api/posts/{id}/sign", post(posts::sign))
        .route("/api/recipes/{id}/prep", get(prep_sheet::handler))
        .route("/api/export", get(export::handler))
        .route("/api/backup", get(export::backup))
        .route("/api/stats", get(export::count))
        .route("/api/import", post(import::handler))
        .route("/api/import/json", post(import::json_handler))
        .route("/api/meal-plans", get(meal_plans::list).post(meal_plans::set_meal))
        .route("/api/meal-plans/clear", post(meal_plans::clear_meal))
        .route(
            "/api/meal-plans/generate-shopping",
            post(meal_plans::generate_shopping),
        )
        .route("/api/shopping", get(shopping::list).post(shopping::add_item))
        .route("/api/shopping/clear-checked", post(shopping::clear_checked))
        .route("/api/shopping/{id}/toggle", post(shopping::toggle))
        .route(
            "/api/shopping/{id}",
            axum::routing::delete(shopping::delete_item),
        )
        // Ingredient master — list/read any auth; write manager only
        .route(
            "/api/ingredients",
            get(ingredients::list).post(ingredients::create),
        )
        .route(
            "/api/ingredients/backfill",
            post(ingredients::backfill),
        )
        .route(
            "/api/ingredients/{id}",
            get(ingredients::show)
                .put(ingredients::update)
                .delete(ingredients::delete),
        )
        .route(
            "/api/ingredients/{id}/recipes",
            get(ingredients::usage),
        )
        .route("/api/tags", get(tags::list))
        .route(
            "/api/recipes/{id}/tags",
            get(tags::recipe_tags).post(tags::add_recipe_tag),
        )
        .route(
            "/api/recipes/{recipe_id}/tags/{tag_id}",
            axum::routing::delete(tags::remove_recipe_tag),
        )
        .route("/api/cookbooks", get(cookbooks::list).post(cookbooks::create))
        .route(
            "/api/cookbooks/{id}/recipes",
            get(cookbooks::recipes).post(cookbooks::add_recipe),
        )
        .route(
            "/api/cookbooks/{id}/recipes/{recipe_id}",
            axum::routing::delete(cookbooks::remove_recipe),
        )
        .fallback_service(ServeDir::new(static_dir))
        .with_state(state)
}

async fn serve_index(index_path: String) -> Result<axum::response::Html<String>, StatusCode> {
    let html = tokio::fs::read_to_string(&index_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(axum::response::Html(html))
}

#[cfg(test)]
mod api_role_tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request},
    };
    use serde_json::{json, Value};
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::AppState;
    use larder_core::db::init_db;

    struct TestApp {
        router: Router,
        _db_path: std::path::PathBuf,
    }

    impl Drop for TestApp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self._db_path);
            let _ = std::fs::remove_file(self._db_path.with_extension("db-wal"));
            let _ = std::fs::remove_file(self._db_path.with_extension("db-shm"));
        }
    }

    async fn test_app() -> TestApp {
        let path = std::env::temp_dir().join(format!("larder-test-{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite:{}", path.display());
        let pool = init_db(&url).await.unwrap();
        TestApp {
            router: create_router(Arc::new(AppState::new(pool))),
            _db_path: path,
        }
    }

    async fn json_request(
        app: &Router,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (u16, Value) {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(t) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        let req = builder
            .body(match body {
                Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
                None => Body::empty(),
            })
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status().as_u16();
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }

    async fn html_request(app: &Router, uri: &str, token: &str) -> (u16, String) {
        let req = Request::builder()
            .method("GET")
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status().as_u16();
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn login(app: &Router, email: &str, password: &str) -> String {
        let (_, body) = json_request(
            app,
            "POST",
            "/api/auth/login",
            None,
            Some(json!({ "email": email, "password": password })),
        )
        .await;
        body["token"].as_str().expect("token").to_string()
    }

    fn any_cost_string(ings: &Value) -> bool {
        ings.as_array().into_iter().flatten().any(|ing| {
            ing.get("cost_per_unit").is_some_and(|v| v.is_string())
                || ing.get("line_cost").is_some_and(|v| v.is_string())
        })
    }

    #[tokio::test]
    async fn kitchen_cannot_see_recipe_or_master_costs() {
        let app = test_app().await;
        let kitchen = login(&app.router, "kitchen@larder.local", "kitchen").await;
        let (_, recipes) = json_request(&app.router, "GET", "/api/recipes", Some(&kitchen), None).await;
        let id = recipes[0]["id"].as_str().expect("recipe id");
        assert!(
            recipes[0].get("menu_price").map(|v| v.is_null()).unwrap_or(true),
            "kitchen recipe list leaked menu_price: {}",
            recipes[0]
        );

        let (_, ings) = json_request(
            &app.router,
            "GET",
            &format!("/api/recipes/{id}/ingredients"),
            Some(&kitchen),
            None,
        )
        .await;
        assert!(
            !any_cost_string(&ings),
            "kitchen recipe ingredients leaked costs: {ings}"
        );

        let (_, masters) = json_request(
            &app.router,
            "GET",
            "/api/ingredients",
            Some(&kitchen),
            None,
        )
        .await;
        assert!(
            !any_cost_string(&masters),
            "kitchen ingredient master leaked costs: {masters}"
        );

        let (status, html) = html_request(
            &app.router,
            &format!("/api/recipes/{id}/prep"),
            &kitchen,
        )
        .await;
        assert_eq!(status, 200);
        assert!(
            !html.contains("Est. food cost") && !html.contains("class=\"cost\""),
            "kitchen prep sheet leaked costs"
        );
    }

    #[tokio::test]
    async fn manager_can_see_costs() {
        let app = test_app().await;
        let manager = login(&app.router, "manager@larder.local", "manager").await;
        let (_, recipes) = json_request(&app.router, "GET", "/api/recipes", Some(&manager), None).await;
        let id = recipes[0]["id"].as_str().expect("recipe id");

        let (_, ings) = json_request(
            &app.router,
            "GET",
            &format!("/api/recipes/{id}/ingredients"),
            Some(&manager),
            None,
        )
        .await;
        assert!(
            any_cost_string(&ings),
            "manager should see recipe line costs: {ings}"
        );

        let (_, masters) = json_request(
            &app.router,
            "GET",
            "/api/ingredients",
            Some(&manager),
            None,
        )
        .await;
        assert!(
            any_cost_string(&masters),
            "manager should see master costs: {masters}"
        );

        let (status, html) = html_request(
            &app.router,
            &format!("/api/recipes/{id}/prep"),
            &manager,
        )
        .await;
        assert_eq!(status, 200);
        assert!(
            html.contains("Est. food cost") || html.contains("class=\"cost\""),
            "manager prep sheet should include costs"
        );
    }

    #[tokio::test]
    async fn import_rejects_rfc1918_urls() {
        let app = test_app().await;
        let manager = login(&app.router, "manager@larder.local", "manager").await;
        let (status, body) = json_request(
            &app.router,
            "POST",
            "/api/import",
            Some(&manager),
            Some(json!({ "url": "http://127.0.0.1/recipe" })),
        )
        .await;
        assert_eq!(status, 400, "ssrf import status, body={body}");
    }
}
