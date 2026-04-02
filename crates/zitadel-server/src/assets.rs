use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::Embed;

/// Embedded frontend assets from web/dist.
/// In dev mode (no web/dist), this will be empty and Vite serves assets instead.
///
/// The folder must exist at compile time. `make ensure-webdist-rs` creates a
/// placeholder if web/dist hasn't been built yet.
#[derive(Embed)]
#[folder = "../../web/dist/"]
struct WebAssets;

pub fn routes() -> Router {
    Router::new()
        // Immutable hashed assets (1 year cache).
        .route("/assets/{*path}", get(serve_asset))
        // SPA fallbacks — serve index.html for client-side routing.
        .route("/login", get(login_spa))
        .route("/login/{*path}", get(login_spa))
        .route("/console", get(console_spa))
        .route("/console/{*path}", get(console_spa))
        .route("/account", get(account_spa))
        .route("/account/{*path}", get(account_spa))
        // Root redirects to console.
        .route("/", get(root_redirect))
}

async fn root_redirect() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, "/console")
        .body(Body::empty())
        .unwrap()
}

async fn login_spa() -> impl IntoResponse {
    serve_spa_index("src/login/index.html")
}

async fn console_spa() -> impl IntoResponse {
    serve_spa_index("src/console/index.html")
}

async fn account_spa() -> impl IntoResponse {
    serve_spa_index("src/account/index.html")
}

fn serve_spa_index(path: &str) -> Response {
    match WebAssets::get(path) {
        Some(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(content.data.to_vec()))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("frontend not embedded (use Vite dev server on :5173)"))
            .unwrap(),
    }
}

async fn serve_asset(request: Request) -> impl IntoResponse {
    let path = request.uri().path();
    // Strip leading slash for rust-embed lookup.
    let path = path.strip_prefix('/').unwrap_or(path);

    match WebAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(
                    header::CACHE_CONTROL,
                    "public, max-age=31536000, immutable",
                )
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("not found"))
            .unwrap(),
    }
}
