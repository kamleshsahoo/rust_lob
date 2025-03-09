use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use axum::body::Bytes;
use futures::lock::Mutex;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use crate::engine::orderbook::{Arena, BidOrAsk};

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
  executed_orders_cnt: i64,
  nos: i64
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

// #[derive(Debug, Deserialize)]
// pub struct LargeUploadRequest {
//   pub session_id: Option<String>,
//   pub total_chunks: usize,
//   pub chunk_number: usize,
//   pub chunk: Vec<u8>
// }

#[derive(Debug, Serialize)]
pub struct LargeUploadResponse {
  pub orderbook_results: Option<HashMap<String, FinalStats>>,
  pub parse_results: Option<(Duration, i32, i32)>,
  // pub session_id: Option<String>,
  pub processed: bool
}

struct LargeUploadSessionData {
  chunks: HashMap<usize, Bytes>,
  // chunk_sender: mpsc::Sender<Vec<u8>>,
  // result_receiver: mpsc::Receiver<(HashMap<String, FinalStats>, (Duration, i32, i32))>,
  // created_at: Instant,
  total_chunks: usize,
  // total_orders: usize,
  // processed_chunks: usize
}


#[derive(Clone)]
pub struct LargeUploadSessionManager {
  sessions: Arc<Mutex<HashMap<String, LargeUploadSessionData>>>
}


impl LargeUploadSessionManager {
  pub fn new() -> Self {
    Self { sessions: Arc::new(Mutex::new(HashMap::new())) }
  }

  pub async fn store_chunk(&self,
    session_id: &str,
    chunk_number: usize,
    chunk_data: Bytes,
    total_chunks: usize
  ) {
    let mut sessions = self.sessions.lock().await;

    let session = sessions.entry(session_id.to_string()).or_insert_with(|| LargeUploadSessionData {
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

  pub async fn get_all_chunks(&self, session_id: &str) -> Result<Vec<u8>, String> {
    let mut sessions = self.sessions.lock().await;

    let session = sessions.get_mut(session_id).ok_or_else(|| format!("Session {} not found", session_id))?;

    if session.chunks.len() != session.total_chunks {
      return Err(format!(
        "Incomplete Upload: got {}/{} chunks",
        session.chunks.len(),
        session.total_chunks
      ));
    }

    // concatenate chunks in order
    let mut result = Vec::new();
    for chunk_num in 0..session.total_chunks {
      if let Some(chunk) = session.chunks.get(&chunk_num) {
        result.extend_from_slice(chunk);
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

  /* May Remove- seems unnecessarily complex
  pub async fn create_session(
    &self,
    session_id: String,
    total_chunks: usize,
    // total_orders: usize
  ) -> mpsc::Sender<Vec<u8>> {

    let (chunk_tx, chunk_rx) = mpsc::channel(1_000);
    let (result_tx, result_rx) = mpsc::channel(1_000);

    let session = LargeUploadSessionData {
      chunk_sender: chunk_tx.clone(),
      result_receiver: result_rx,
      created_at: Instant::now(),
      total_chunks,
    };

    let mut sessions = self.sessions.lock().await;
    sessions.insert(session_id, session);

    Self::spawn_processing_task(chunk_rx, result_tx, total_chunks);

    chunk_tx
  }

  fn spawn_processing_task(
    mut chunk_rx: mpsc::Receiver<Vec<u8>>,
    result_tx: mpsc::Sender<(HashMap<String, FinalStats>, (Duration, i32, i32))>,
    total_chunks: usize
  ) {
    tokio::spawn(async move {
      //let mut accumulator = Vec::new();
      let mut processed_chunks = 0;
      let mut total_parsed_orders: Vec<FileUploadOrderType> = Vec::with_capacity(10_000_000);
      let mut total_duration: Duration = Duration::new(0, 0);
      let mut total_raw_orders: i32 = 0;
      let mut total_invalid_orders: i32 = 0;

      while let Some(chunk) = chunk_rx.recv().await {
        let file_contents = String::from_utf8(chunk).expect("failed to get file contents as String!");
        let (parsed_orders, duration, raw_cnt, invalid_cnt) = parse_file_orders(file_contents);
        total_parsed_orders.extend(parsed_orders);
        total_duration += duration;
        total_raw_orders += raw_cnt;
        total_invalid_orders += invalid_cnt;
        //accumulator.extend(chunk);
        processed_chunks += 1;

        if processed_chunks >= total_chunks {
          //let file_contents = String::from_utf8(accumulator).expect("failed to get file contents as Strung!");
          //let (parsed_orders, duration, raw_cnt, invalid_cnt) = parse_file_orders(file_contents);
          let result = process_uploaded_orders(total_parsed_orders).await;
          result_tx.send((result, (total_duration, total_raw_orders, total_invalid_orders))).await.expect("failed to send final result for largefile on channel");
          break;
        }
      }
    });
  }

  pub async fn get_chunk_sender(&self, session_id: &str) -> Option<mpsc::Sender<Vec<u8>>> {
    let sessions = self.sessions.lock().await;
    sessions.get(session_id).map(|s| s.chunk_sender.clone())
  }

  pub async fn take_result_receiver(&self, session_id: &str) -> Option<mpsc::Receiver<(HashMap<String, FinalStats>, (Duration, i32, i32))>> {
    let mut sessions = self.sessions.lock().await;
    sessions.remove(session_id).map(|s| s.result_receiver)
  }
  */
}



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