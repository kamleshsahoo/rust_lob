use std::{collections::HashMap, fmt, time::Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::pages::simulator::{EngineStats, ExecutedOrders};
use super::file_handler::{FileUploadOrderType, FinalStats};

/* Server Responses */
#[derive(Debug, Deserialize)]
pub struct HealthCheckResponse {
  pub code: i32,
  pub status: String
}

#[derive(Debug, Deserialize)]
pub struct SignedUrlResponse {
  pub signed_url: String
}

#[derive(Debug, Deserialize)]
// #[serde(tag = "type")]
pub enum ServerMessage {
    PriceLevels { snapshot: bool, bids: Vec<(Decimal, u64)>, asks: Vec<(Decimal, u64)> },
    Trades (Vec<ExecutedOrders>),
    // EngineStats(Vec<EngineStats>)
    ExecutionStats (EngineStats),
    BestLevels {best_buy: Option<Decimal>, best_sell: Option<Decimal>},
    Completed
}

#[derive(Debug, Deserialize)]
pub struct UploadResponse {
  // success: bool,
  pub data: Option<HashMap<String, FinalStats>>,
  pub session_id: Option<String>,
  // error_msg: Option<String>  
}

#[derive(Debug, Deserialize)]
pub struct LargeUploadResponse {
  pub orderbook_results: HashMap<String, FinalStats>,
  pub parse_results: (Duration, i32, i32)
}


/* Server Requests */
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
Start { 
    // client_name: String, 
    total_objects: usize,  // Optional, defaults to 10
    throttle_nanos: u64, // Optional, defaults to 1000ns
    mean_price: f64,  // Optional, defaults to 300.0
    sd_price: f64,  // Optional, defaults to 50.0
    best_price_levels: bool // whether to show best bids and asks, defaults to false
},
Stop,
Ack
}

#[derive(Debug, Serialize)]
pub struct UploadRequest {
  pub session_id: Option<String>,
  pub total_chunks: usize,
  pub total_orders: usize,
  pub chunk_number: usize,
  pub orders: Vec<FileUploadOrderType>
}

/* Server Error */
#[derive(Debug, Clone, PartialEq)]
pub enum ServerError {
    ConnectionFailed(String),
    ServerUnhealthy(String)
}

impl std::error::Error for ServerError {}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            ServerError::ServerUnhealthy(msg) => write!(f, "Server unhealthy: {}", msg)
        }
    }
}