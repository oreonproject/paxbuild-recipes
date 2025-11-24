use axum::response::Html;
use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::services::ServeFile;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8082".to_string());
    let addr = SocketAddr::from(([0, 0, 0, 0], port.parse()?));

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/", get(dashboard_page))
        .route_service("/OSAD.png", ServeFile::new("OSAD.png"))
        .fallback(fallback_404);

    println!("OSAD Read-only Dashboard listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

async fn healthz() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

async fn dashboard_page() -> impl IntoResponse {
    Html(include_str!("static/osad_public.html"))
}

async fn fallback_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not Found")
}
