pub mod account;
pub mod oidc;
pub mod planner;
pub mod recipes;
pub mod transfer;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::error::AppError;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/config", get(account::public_config))
        .route("/auth/register", post(account::register))
        .route("/auth/login", post(account::login))
        .route("/auth/refresh", post(account::refresh))
        .route("/auth/logout", post(account::logout))
        .route("/auth/oidc/login", get(oidc::login))
        .route("/auth/oidc/callback", get(oidc::callback))
        .route("/users/me", get(account::me).patch(account::update_me))
        .route("/household", get(account::get_household).patch(account::update_household))
        .route("/household/invites", get(account::list_invites).post(account::create_invite))
        .route("/household/invites/{code}", delete(account::delete_invite))
        .route("/household/members/{id}", patch(account::update_member).delete(account::remove_member))
        .route("/admin/users", get(account::admin_list_users))
        .route("/admin/users/{id}", patch(account::admin_update_user).delete(account::admin_delete_user))
        .route("/recipes", get(recipes::list).post(recipes::create))
        .route("/recipes/organize", post(recipes::organize))
        .route("/recipes/{id}", get(recipes::get).put(recipes::update).delete(recipes::delete))
        .route("/recipes/{id}/duplicate", post(recipes::duplicate))
        .route("/recipes/{id}/favorite", put(recipes::favorite).delete(recipes::unfavorite))
        .route("/recipes/{id}/image", put(recipes::upload_image).post(recipes::upload_image).delete(recipes::delete_image))
        .route("/images/{id}", get(recipes::serve_image))
        .route("/tags", get(recipes::tags))
        .route("/categories", get(recipes::categories))
        .route("/tags/rename", post(recipes::rename_tag))
        .route("/tags/delete", post(recipes::delete_tag))
        .route("/parse/ingredients", post(recipes::parse_ingredients))
        .route("/sync", get(planner::sync))
        .route("/mealplan", get(planner::list_meal_plan).post(planner::create_meal_plan))
        .route("/mealplan/{id}", put(planner::update_meal_plan).delete(planner::delete_meal_plan))
        .route("/shopping-lists", get(planner::list_lists).post(planner::create_list))
        .route("/shopping-lists/{id}", get(planner::get_list).patch(planner::update_list).delete(planner::delete_list))
        .route("/shopping-lists/{id}/items", post(planner::add_item))
        .route("/shopping-lists/{id}/items/{item_id}", patch(planner::update_item).delete(planner::delete_item))
        .route("/shopping-lists/{id}/clear-checked", post(planner::clear_checked))
        .route("/shopping-lists/{id}/add-recipes", post(planner::add_recipes))
        .route("/shopping-lists/{id}/add-mealplan", post(planner::add_mealplan))
        .merge(transfer::routes())
        .fallback(|| async { AppError::not_found("no such API endpoint") })
        .layer(DefaultBodyLimit::max(state.config.max_upload_bytes))
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-store")));

    let mut app = Router::new().nest("/api", api).route("/healthz", get(|| async { (StatusCode::OK, "ok") }));

    if let Some(web) = &state.config.web_dir {
        // Serve the Flutter web build; unknown paths fall back to index.html (SPA routing).
        let index = web.join("index.html");
        let serve = ServeDir::new(web).fallback(ServeFile::new(index));
        app = app.fallback_service(serve);
    } else {
        app = app.fallback(|| async {
            (StatusCode::NOT_FOUND, "Larder API server. The web UI is not installed (set LARDER_WEB_DIR).").into_response()
        });
    }

    let cors = if state.config.cors_origins.is_empty() {
        None
    } else {
        let origins: Vec<HeaderValue> = state.config.cors_origins.iter().filter_map(|o| o.parse().ok()).collect();
        Some(
            CorsLayer::new()
                .allow_origin(AllowOrigin::list(origins))
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
        )
    };

    let app = app.layer(CompressionLayer::new()).layer(TraceLayer::new_for_http()).with_state(state);
    match cors {
        Some(c) => app.layer(c),
        None => app,
    }
}
