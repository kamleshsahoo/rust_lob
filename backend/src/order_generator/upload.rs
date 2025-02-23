use std::{collections::HashMap, time::{Duration, Instant}};
use axum::{http::StatusCode, response::IntoResponse};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::engine::orderbook::{Arena, BidOrAsk};


pub enum UploadError {
  DeserializeError(String),
  ChannelError(String),
  InvalidChunk,
  SessionNotFound
}

impl IntoResponse for UploadError {
  fn into_response(self) -> axum::response::Response {
    let body = match self {
      UploadError::DeserializeError(cause) => cause,
      UploadError::ChannelError(cause) => cause,
      UploadError::InvalidChunk => "Invalid chunk number".to_string(),
      UploadError::SessionNotFound => "session not found for getting channels".to_string()
    };

    (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
  }
}

#[derive(Debug, Deserialize)]
pub enum FileUploadOrderType {
  Add {
    id: u64,
    side: BidOrAsk,
    shares: u64,
    price: Decimal
  },
  Modify {
    id: u64,
    shares: u64,
    price: Decimal
  },
  Cancel {
    id: u64,
  },
}

struct OrderStats {
  latency: Duration,
  avl_rebalances: i64,
  executed_orders_cnt: usize
}

#[derive(Debug, Serialize)]
pub struct FinalStats {
  total_time: Duration,
  avl_rebalances: i64,
  executed_orders_cnt: i64
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
  pub data: Option<HashMap<String, FinalStats>>,
  pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UploadRequest {
  pub session_id: Option<String>,
  pub total_chunks: usize,
  pub total_orders: usize,
  pub chunk_number: usize,
  pub orders: Vec<FileUploadOrderType>
}

pub async fn process_upload_orders(orders: Vec<FileUploadOrderType>) -> HashMap<String, FinalStats>  {

  println!("[INFO] processing total {:?} orders", orders.len());
  let mut book = Arena::new(true);
  let mut order_stats = HashMap::new();

  for order in orders {
    
    match order {
      FileUploadOrderType::Add { id, side, shares, price } => {
        let start = Instant::now();
        book.add_limit_order(id, side, shares, price);
        let duration = start.elapsed();
        order_stats.entry("ADD")
        .or_insert(vec![])
        .push(OrderStats { latency: duration, avl_rebalances: book.avl_rebalances as i64, executed_orders_cnt: book.executed_orders_count });
      },
      FileUploadOrderType::Modify { id, shares, price } => {
        let start = Instant::now();
        book.modify_limit_order(id, shares, price);
        let duration = start.elapsed();
        order_stats.entry("MODIFY")
        .or_insert(vec![])
        .push(OrderStats { latency: duration, avl_rebalances: book.avl_rebalances as i64, executed_orders_cnt: book.executed_orders_count });
      },
      FileUploadOrderType::Cancel { id } => {
        let start = Instant::now();
        book.cancel_limit_order(id);
        let duration = start.elapsed();
        order_stats.entry("CANCEL")
        .or_insert(vec![])
        .push(OrderStats { latency: duration, avl_rebalances: book.avl_rebalances as i64, executed_orders_cnt: book.executed_orders_count });
      }
    }
  }
  
  let mut final_stats: HashMap<String, FinalStats> = HashMap::new();
  
  for (k, v) in order_stats {
    let z = v.iter().fold(FinalStats { total_time: Duration::new(0, 0), avl_rebalances: 0, executed_orders_cnt: 0 }, |mut state, e| {
      state.total_time += e.latency;
      state.avl_rebalances += e.avl_rebalances;
      state.executed_orders_cnt += e.executed_orders_cnt as i64;
      state
    });
    final_stats.insert(k.to_string(), z);
  }
  
  final_stats
}