mod order_generator;
mod engine;
mod file_upload;
mod midwares;
mod route_handlers;

use std::net::SocketAddr;
use axum::{
  extract::DefaultBodyLimit,
  http::{header::{CONTENT_ENCODING, CONTENT_TYPE}, HeaderName, HeaderValue, Method},
  middleware,
  routing::{any, get, post},
  Extension,
  Json,
  Router
};
use serde_json::json;
use tokio::{net::TcpListener, sync::OnceCell};
use tower_http::cors::CorsLayer;

use file_upload::processor::{LargeUploadSessionManager, SmallUploadSessionManager};
use midwares::{app_state::{PostgresDBPool, RateLimiter}, auth::ip_tracker_with_auth};
use route_handlers::{sockets::ws_handler, uploads::{large_upload_handler, small_upload_handler}};

// allow max file uploads of 15MB for the /largeupload route
const MAX_FILE_SIZE: usize = 1024 * 1024 * 15;
pub static EXPECTED_ORIGIN: OnceCell<String> = OnceCell::const_new();
static REDIS_URL: OnceCell<String> = OnceCell::const_new();
static DB_URL: OnceCell<String> = OnceCell::const_new();
static IP_LIMIT: OnceCell<usize> = OnceCell::const_new();
static IP_WINDOW: OnceCell<i64> = OnceCell::const_new();
static GLOBAL_LIMIT: OnceCell<usize> = OnceCell::const_new();
static GLOBAL_WINDOW: OnceCell<i64> = OnceCell::const_new();

pub async fn get_origin() -> String {
  std::env::var("ORIGIN").expect("EXPECTED ORIGIN should be available!")
}
async fn get_redis() -> String {
  std::env::var("REDIS").expect("REDIS URL should be available!")
}
async fn get_postgres() -> String {
  std::env::var("POSTGRES").expect("POSTGRES URL should be available!")
}
async fn get_ip_limit() -> usize {
  std::env::var("IP_LIMIT").expect("IP_LIMIT should be available!").parse::<usize>().expect("ip limit parse should not fail!")
}
async fn get_ip_window() -> i64 {
  std::env::var("IP_WINDOW").expect("IP_WINDOW should be available!").parse::<i64>().expect("ip limit parse should not fail!")
}
async fn get_global_limit() -> usize {
  std::env::var("GLOBAL_LIMIT").expect("GLOBAL_LIMIT should be available!").parse::<usize>().expect("ip limit parse should not fail!")
}
async fn get_global_window() -> i64 {
  std::env::var("GLOBAL_WINDOW").expect("GLOBAL_WINDOW should be available!").parse::<i64>().expect("ip limit parse should not fail!")
}


#[tokio::main]
async fn main() {

  let expected_origin = EXPECTED_ORIGIN.get_or_init(get_origin).await;
  let redis_url = REDIS_URL.get_or_init(get_redis).await;
  let db_url = DB_URL.get_or_init(get_postgres).await;
  let ip_limit = IP_LIMIT.get_or_init(get_ip_limit).await;
  let ip_window = IP_WINDOW.get_or_init(get_ip_window).await;
  let global_limit = GLOBAL_LIMIT.get_or_init(get_global_limit).await;
  let global_window = GLOBAL_WINDOW.get_or_init(get_global_window).await;

  let rate_limiter = RateLimiter::new(redis_url, *ip_limit, *ip_window, *global_limit, *global_window).expect("failed to create ratelimiterl!");
  let db_pool = PostgresDBPool::new(db_url).await.expect("failed to create postgres connection pool!");

  let small_upload_session_manager = SmallUploadSessionManager::new();
  let large_upload_session_manager = LargeUploadSessionManager::new();

  let cors = CorsLayer::new()
  .allow_methods([Method::GET, Method::POST])
  .allow_origin(expected_origin.parse::<HeaderValue>().unwrap())
  .allow_headers([
    CONTENT_TYPE,
    CONTENT_ENCODING,
    HeaderName::from_static("x-timestamp"),
    HeaderName::from_static("x-signature"),
    ]);
    
  let with_middleware = Router::new()
    .route("/wslob", any(ws_handler))
    .route("/smallupload", post(small_upload_handler)
              .with_state(small_upload_session_manager))
    .route("/largeupload", post(large_upload_handler)
            .layer(DefaultBodyLimit::max(MAX_FILE_SIZE))
            .with_state(large_upload_session_manager))
    .layer(Extension(rate_limiter))
    .layer(Extension(db_pool))
    .layer(middleware::from_fn(ip_tracker_with_auth));

  let health_check = Router::new()
    .route("/health", get(health_check_handler));

  let app = Router::new()
    .merge(with_middleware)
    .merge(health_check)
    .layer(cors);

  let listener = TcpListener::bind("0.0.0.0:7575").await.expect("failed to start tcp listener");

  axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.expect("failed to start server");
}

async fn health_check_handler() -> Json<serde_json::Value> {
  Json(json!({"code":200, "status": "healthy"}))
}