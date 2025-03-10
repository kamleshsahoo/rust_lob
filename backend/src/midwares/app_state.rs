use std::{net::SocketAddr, sync::Arc};
use axum::{
  body::Bytes, extract::{ConnectInfo, Request}, http::StatusCode, middleware::Next, response::{IntoResponse, Response}, Json 
};
use serde::Serialize;
use serde_json::json;
use redis::{AsyncCommands, Client as RedisClient};
use sqlx::{postgres::PgPoolOptions, PgPool};

// simple heurestic based on size to estimate orders 
static ESTIMATED_ORDERS_PER_MB: i32 = 35_000;

// Request context containing IP information
#[derive(Clone)]
pub struct RequestContext {
    pub remote_ip: String,
    pub server_ip: String,
}


#[derive(Debug, Serialize, Clone)]
pub enum AppError {
  RateLimitExceeded(String),
  DeserializeError(String),
  BadRequest(String),
  InternalError(String),
  // WebSocketError(String),
}

impl IntoResponse for AppError {
  fn into_response(self) -> axum::response::Response {
    let (status, message) = match self {
      Self::RateLimitExceeded(msg) => (StatusCode::TOO_MANY_REQUESTS, msg),
      Self::DeserializeError(msg) => (StatusCode::BAD_REQUEST, msg),
      Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
      Self::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
      // Self::WebSocketError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
    };

    let body = Json(json!({"error": message, "code": status.as_u16()}));

    (status, body).into_response()
  }
}

#[derive(Clone)]
pub struct RateLimiter {
  redis: Arc<RedisClient>,
  ip_max_orders: usize,
  ip_window_secs: i64,
  global_max_orders: usize,
  global_window_secs: i64
}

impl RateLimiter {
  pub fn new(redis_url: &str) -> Result<Self, AppError> {
    println!("[INFO] Redis instance initializing for rate limiter...");

    let client = RedisClient::open(redis_url)
      .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

    Ok(Self {
      redis: Arc::new(client),
      ip_max_orders: 2_000_000,
      ip_window_secs: 30, 
      global_max_orders: 3_000_000,
      global_window_secs: 2 * 60  
    })
  }

  pub async fn would_exceed_limit(&self, ip: &str, orders: &usize) -> Result<(), AppError> {
    let mut conn = self.redis.get_multiplexed_async_connection().await
    .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

    let ip_key = format!("rate:ip:{}", ip);
    let ip_count: usize = conn.get(&ip_key).await.unwrap_or(0);

    // Check IP-level rate limit
    if ip_count + orders > self.ip_max_orders {
      return Err(AppError::RateLimitExceeded(
        format!("IP address has exceeded the limit of {} orders in a {} hour window",
        self.ip_max_orders, self.ip_window_secs / 3600
        )
      ));
    }

    // Check global app-level rate limit
    let global_key = "rate:global";
    let global_count: usize = conn.get(global_key).await.unwrap_or(0);

    if global_count + orders > self.global_max_orders {
      return Err(AppError::RateLimitExceeded(
        format!("Application has exceeded the limit of {} orders in a {} hour window",
        self.global_max_orders, self.global_window_secs / 3600
        )
      ));
    }

    Ok(())

  }

  pub async fn record_orders(&self, ip: &str, orders: usize) -> Result<(), AppError> {
    let mut conn = self.redis.get_multiplexed_async_connection().await
    .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

    // Record IP-level counter
    let ip_key = format!("rate:ip:{}", ip);
    let incr_res: usize = conn.incr(&ip_key, orders).await.map_err(|e| AppError::InternalError(format!("Redis operation failed: {}", e)))?;
    // incr returns the delta if key was not present
    if incr_res == orders {
      // set expiration if key was created
      let _: () = conn.expire(&ip_key, self.ip_window_secs).await.map_err(|e| AppError::InternalError(format!("Redis operation failed: {}", e)))?;
    }

    // Record global counter
    let global_key = "rate:global";
    let glob_incr_res: usize = conn.incr(global_key, orders).await.map_err(|e| AppError::InternalError(format!("Redis operation failed: {}", e)))?;
    if glob_incr_res == orders {
      // set expiration if key was created
      let _: () = conn.expire(global_key, self.global_window_secs).await.map_err(|e| AppError::InternalError(format!("Redis operation failed: {}", e)))?;
    }
    Ok(())
  }

}

#[derive(Clone)]
pub struct PostgresDBPool {
  pool: PgPool
}

impl PostgresDBPool {
  pub async fn new(db_url: &str) -> Result<Self, AppError> {
    println!("[INFO] Postgres db pool initializing..");

    let pool = PgPoolOptions::new()
      .max_connections(8)
      .connect(db_url)
      .await
      .map_err(|e| AppError::InternalError(format!("Postgres connection error: {}", e)))?;

    println!("[**]created db pool!");
    Self::init_table(&pool).await?;

    Ok(Self { pool })

  }

  async fn init_table(pool: &PgPool) -> Result<(), AppError> {
    sqlx::raw_sql(
      "
      CREATE TABLE IF NOT EXISTS client_daily_visits (
        id SERIAL PRIMARY KEY,
        remote_ip TEXT NOT NULL,
        server_ip TEXT NOT NULL,
        visit_date DATE NOT NULL DEFAULT CURRENT_DATE,
        last_visit TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
        rl_visits INT NOT NULL DEFAULT 0,
        total_visits INT NOT NULL DEFAULT 0,
        total_orders BIGINT NOT NULL DEFAULT 0,
        UNIQUE(remote_ip, visit_date)
      );
      "
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalError(format!("Failed to create Postgres table: {}", e)))?;
    
    Ok(())
  }
  
  pub fn record_in_db(
    &self,
    remote_ip: &str,
    server_ip: &str,
    total_orders: usize,
    was_rate_limited: bool
  ) {
    let pool = self.pool.clone();

    let remote_ip = remote_ip.to_string();
    let server_ip = server_ip.to_string();
    let rl_visits: usize = if was_rate_limited { 1 } else { 0 };

    // async store in DB
    tokio::spawn(async move {

      let query = format!(
        "
        INSERT INTO client_daily_visits (remote_ip, server_ip, visit_date, last_visit, rl_visits, total_visits, total_orders)
        VALUES ('{}', '{}', CURRENT_DATE, CURRENT_TIMESTAMP, {}, 1, {})
        ON CONFLICT (remote_ip, visit_date) DO UPDATE
        SET last_visit = CURRENT_TIMESTAMP,
            rl_visits = client_daily_visits.rl_visits + EXCLUDED.rl_visits,
            total_visits = client_daily_visits.total_visits + 1,
            total_orders = client_daily_visits.total_orders + EXCLUDED.total_orders;
        ",
        remote_ip,
        server_ip,
        rl_visits,
        total_orders
      );

      let query_result = sqlx::raw_sql(&query)
        .execute(&pool)
        .await;
        // .map_err(|e| AppError::InternalError(format!("Failed to create Postgres table: {}", e)));

      if let Err(e) = query_result {
        println!("Failed to execute upsert query: {}", e);
      }
    });
  }
}

// IP tracking Middleware
pub async fn ip_tracker(
  ConnectInfo(addr): ConnectInfo<SocketAddr>,
  mut req: Request,
  next: Next
) -> Response {

  //println!("[IPtracker MW] got headers");
  // for (k, v) in req.headers().iter() {
  //   println!("{:?}: {:?}", k, v);
  // }

  // Get remote IP from GCP headers, or fallback to socket address
  let remote_ip = req.headers()
    .get("X-Forwarded-For")
    .and_then(|h| h.to_str().ok())
    .unwrap_or(&addr.ip().to_string())
    .to_string();
  println!("[**]remote_ip: {}", &remote_ip);

  let server_ip = req.headers()
    .get("X-Forwarded-Server")
    .and_then(|h| h.to_str().ok())
    .unwrap_or("NA")
    .to_string();
  println!("[**]server_ip: {}", &server_ip);

  req.extensions_mut().insert(RequestContext {
    remote_ip,
    server_ip
  });

  next.run(req).await
}

pub fn estimate_orders_from_1stchunk(chunk_data: &Bytes, total_chunks: &usize) -> usize {

  let chunk_size_mb = (chunk_data.len() as f64) / (1024.0 * 1024.0);
  let estimated_orders_in_chunk = (chunk_size_mb * ESTIMATED_ORDERS_PER_MB as f64) as usize;

 estimated_orders_in_chunk * total_chunks
}