use std::{collections::HashMap, fmt, time::Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::pages::simulator::{EngineStats, ExecutedOrders};
use super::file_handler::{FileUploadOrderType, FinalStats};

/* Server Requests */
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum WsRequest {
  Start { 
    total_objects: usize,  //defaults to 50_000
    mean_price: f64,  //defaults to 250.0
    sd_price: f64,  // defaults to 20.0
    order_probs: Vec<f32>, //probs for [ADD, CANCEL, MODIFY] defaults to [0.0, 0.4 ,0.6] 
    best_price_levels: bool // defaults to false
  },
  Stop,
  Ack
}

#[derive(Debug, Serialize)]
pub struct SmallUploadRequest {
  pub session_id: String,
  pub total_chunks: usize,
  pub total_orders: usize,
  pub chunk_number: usize,
  pub orders: Vec<FileUploadOrderType>
}

/* Server Responses */
#[derive(Debug, Deserialize)]
pub struct HealthCheckResponse {
  pub code: i32,
  pub status: String
}

#[derive(Debug, Deserialize)]
pub enum WsResponse {
    PriceLevels { snapshot: bool, bids: Vec<(Decimal, u64)>, asks: Vec<(Decimal, u64)> },
    Trades (Vec<ExecutedOrders>),
    ExecutionStats (EngineStats),
    BestLevels {best_buy: Option<Decimal>, best_sell: Option<Decimal>},
    Completed,
    RateLimitExceeded
}

#[derive(Debug, Deserialize)]
pub struct SmallUploadResponse {
  pub orderbook_results: Option<HashMap<String, FinalStats>>,
  pub processed: bool
}

#[derive(Debug, Deserialize)]
pub struct LargeUploadResponse {
  pub orderbook_results: Option<HashMap<String, FinalStats>>,
  pub parse_results: Option<(Duration, i32, i32)>,
  pub processed: bool
}

// App Errors
#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
  WsConnectionError(String),
  UploadConnectionError(String),
  ServerUnhealthy(String),
  RateLimitExceeded(String),
  CompressionError(String),
  DecompressionError(String),
  WsChannelError(String),
  ReqwestError(String),
  SerializeError(String),
  DeserializeError(String),
  WasmError(String),
  AuthorizationError(String),
}

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      AppError::WsConnectionError(msg) => write!(f, "Websocket connection error: {}", msg),
      AppError::UploadConnectionError(msg) => write!(f, "Upload connection error: {}", msg),
      AppError::ServerUnhealthy(msg) => write!(f, "Server unhealthy: {}", msg),
      AppError::RateLimitExceeded(msg) => write!(f, "Rate limit exceeded: {}", msg),
      AppError::CompressionError(msg) => write!(f, "Compression error: {}", msg),
      AppError::DecompressionError(msg) => write!(f, "Decompression error: {}", msg),
      AppError::WsChannelError(msg) => write!(f, "Websocket update channel error: {}", msg),
      AppError::ReqwestError(msg) => write!(f, "Reqwest error: {}", msg),
      AppError::SerializeError(msg) => write!(f, "Serialize error :{}", msg),
      AppError::DeserializeError(msg) => write!(f, "Deserialize error:{}", msg),
      AppError::WasmError(msg) => write!(f, "Wasm error: {}", msg),
      AppError::AuthorizationError(msg) => write!(f, "Authorization error: {}", msg)
    }
  }
}