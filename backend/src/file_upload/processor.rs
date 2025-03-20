use std::{collections::HashMap, io::Read, sync::Arc, time::{Duration, Instant}};
use axum::body::Bytes;
use flate2::read::DeflateDecoder;
use futures::lock::Mutex;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use crate::{engine::orderbook::{Arena, BidOrAsk}, midwares::app_state::AppError};

#[derive(Debug, Clone, Deserialize)]
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
  executed_orders_cnt: i64,
  nos: i64
}

#[derive(Debug, Serialize)]
pub struct SmallUploadResponse {
  pub orderbook_results: Option<HashMap<String, FinalStats>>,
  pub processed: bool
}

#[derive(Debug, Deserialize)]
pub struct SmallUploadRequest {
  pub session_id: String,
  pub total_chunks: usize,
  pub total_orders: usize,
  pub chunk_number: usize,
  pub orders: Vec<FileUploadOrderType>
}

#[derive(Debug, Serialize)]
pub struct LargeUploadResponse {
  pub orderbook_results: Option<HashMap<String, FinalStats>>,
  pub parse_results: Option<(Duration, i32, i32)>,
  pub processed: bool
}

// Session manager
struct UploadSessionData<T> {
  chunks: HashMap<usize, T>,
  total_chunks: usize
}

#[derive(Clone)]
pub struct UploadSessionManager<T> {
  sessions: Arc<Mutex<HashMap<String, UploadSessionData<T>>>>
}

impl<T, Item> UploadSessionManager<T>
where
  T: Clone + IntoIterator<Item = Item>,
  Vec<Item>: FromIterator<Item>
{
  pub fn new() -> Self {
    Self { sessions: Arc::new(Mutex::new(HashMap::new())) }
  }

  pub async fn store_chunk(&self, session_id: &str, chunk_number: usize, chunk_data: T, total_chunks: usize) {

    let mut sessions = self.sessions.lock().await;
    let session = sessions.entry(session_id.to_string()).or_insert_with(|| UploadSessionData {
      chunks: HashMap::new(),
      total_chunks
    });
    // update session
    session.chunks.insert(chunk_number, chunk_data);
  }

  pub async fn is_upload_complete(&self, session_id: &str) -> bool {
    let sessions = self.sessions.lock().await;

    if let Some(session) = sessions.get(session_id) {
      session.chunks.len() == session.total_chunks
    } else {
      false
    }
  }

  pub async fn get_all_chunks(&self, session_id: &str) -> Result<Vec<Item>, String> {
    let mut sessions = self.sessions.lock().await;

    let session = sessions.get_mut(session_id).ok_or_else(|| format!("Session {} not found", session_id))?;

    if session.chunks.len() != session.total_chunks {
      return Err(format!(
        "Incomplete Upload: got {}/{} chunks",
        session.chunks.len(),
        session.total_chunks
      ));
    }

    // collect chunks in order
    let mut result = Vec::new();
    for chunk_num in 0..session.total_chunks {
      if let Some(chunk) = session.chunks.get(&chunk_num) {
        result.extend(chunk.clone());
      } else {
        return Err(format!("Missing chunk {} in session {}", chunk_num, session_id));
      }
    }

    Ok(result)
  }

  pub async fn clear_chunks(&self, session_id: &str) -> Result<(), String> {
    let mut sessions = self.sessions.lock().await;
    if sessions.remove(session_id).is_none() {
      return Err(format!("Session {} not found for deletion", session_id));
    }
    Ok(())
  }
}

// Type aliases for convenience
pub type LargeUploadSessionManager = UploadSessionManager<Bytes>;
pub type SmallUploadSessionManager = UploadSessionManager<Vec<FileUploadOrderType>>;

// method that feeds uploaded orders into the ob engine (used by both: /largeupload and /smallupload routes)
pub fn process_uploaded_orders(orders: Vec<FileUploadOrderType>) -> HashMap<String, FinalStats>  {

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
    let z = v.iter().fold(FinalStats { total_time: Duration::new(0, 0), avl_rebalances: 0, executed_orders_cnt: 0, nos: 0 }, |mut state, e| {
      state.total_time += e.latency;
      state.avl_rebalances += e.avl_rebalances;
      state.executed_orders_cnt += e.executed_orders_cnt as i64;
      state.nos += 1;
      state
    });
    final_stats.insert(k.to_string(), z);
  }
  
  final_stats
}

pub fn decompress_if_needed(data: &[u8], content_encoding: Option<&str>) -> Result<Vec<u8>, AppError> {
  match content_encoding {
    Some("deflate") => {
      let mut decoder = DeflateDecoder::new(data);
      let mut decompressed_data = Vec::new();
      decoder.read_to_end(&mut decompressed_data).map_err(|e| AppError::BadRequest(e.to_string()))?;
      Ok(decompressed_data)
    },
    _ => Ok(data.to_vec())
  }
}