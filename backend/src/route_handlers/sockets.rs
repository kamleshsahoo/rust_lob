use std::{io::Write, time::Duration};
use axum::{
  extract::{ws::{self, CloseFrame, Message, Utf8Bytes, WebSocket}, WebSocketUpgrade}, response::IntoResponse, Extension
};
use flate2::{write::DeflateEncoder, Compression};
use futures::stream::SplitSink;
use tokio::{sync::mpsc, time::sleep};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;

use crate::{midwares::app_state::{AppError, PostgresDBPool, RateLimiter, RequestContext}, order_generator::gen::{Simulator, WsResponse}};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum WsRequest {
  Start {
    total_objects: usize,  //defaults to 50_000
    mean_price: f64,  //defaults to 300.0
    sd_price: f64,  // defaults to 50.0
    order_probs: Vec<f32>, //probs for [ADD, CANCEL, MODIFY] defaults to [0.0, 0.4 ,0.6] 
    best_price_levels: bool // whether to show best bids and asks, defaults to false
  },
  Stop,
  Ack
}

enum Simulation {
  Start(Vec<WsResponse>),
  Data(Vec<WsResponse>),
  Complete
}

pub async fn ws_handler (
  ws: WebSocketUpgrade,
  Extension(rate_limiter): Extension<RateLimiter>,
  Extension(postgres): Extension<PostgresDBPool>,
  Extension(ctx): Extension<RequestContext>,
) -> impl IntoResponse {

  let RequestContext { remote_ip, origin, user_agent, timestamp, signature } = ctx;

  ws.protocols([signature.to_owned(), timestamp.to_owned()])
    .on_upgrade(move|socket| handle_socket(socket,  rate_limiter, postgres, remote_ip, origin, user_agent))
}

async fn handle_socket(
  socket: WebSocket,
  rate_limiter: RateLimiter,
  postgres: PostgresDBPool,
  who: String,
  origin: String,
  user_agent: String
) {

  let (mut sender, mut receiver) = socket.split();
  let (tx, mut rx) = mpsc::channel(1_000_000);

  let mut batch: Vec<Vec<WsResponse>>  = vec![];
  let mut batch_count: usize = 0;
  const BATCH_SIZE: usize = 500;
  const BREATHE_AFTER_BATCHES: usize = 10;
  const BREATHING_TIME_MS: u64 = 1;

  // Add flag to track if client supports compression
  let mut use_compression = false;

  loop { 
    tokio::select! {
      msg = receiver.next() => {
        if let Some(Ok(client_msg)) = msg {
          match client_msg {
            Message::Text(t) => {
              println!(">>> {} sent string: {:?}", &who, t);

              let payload = serde_json::from_str::<WsRequest>(t.as_str()).expect("derserializng client message failed");      
            
              match payload {
                WsRequest::Start {total_objects, mean_price, sd_price, order_probs, best_price_levels, } => {
                  println!("client payload\ntotal orders: {:?} mean: {:?} sd: {:?} show best price levels: {:?} order probs: {:?}", total_objects, mean_price, sd_price, best_price_levels, order_probs);
                  
                  // for now enable compression for all clients
                  use_compression = true;

                  if let Err(e) = rate_limiter.would_exceed_limit(&who, &total_objects).await {
                    println!("{:?}", e);
                    
                    // send a msg to client and close the connection
                    let rl_exceeded_signal = WsResponse::RateLimitExceeded;
                    let rl_exceeded_msg = Message::text(serde_json::to_string(&vec![[rl_exceeded_signal]]).expect("serializing rate limit exceeded signal failed!"));
                    
                    if let Err(e) = sender.send(rl_exceeded_msg).await {
                      println!("sending rate limit exceeded message to client failed with: {:?}", e);
                    }
                    // log the ratelimited visit in db
                    postgres.record_in_db(&who, &origin, &user_agent, 0, true);

                    graceful_ws_closure(sender, ws::close_code::SIZE, "max order limit reached").await;
                    // finally exit since rate limit was exceeded
                    break;
                  }
                  // log in db
                  postgres.record_in_db(&who, &origin, &user_agent, total_objects, false);
                  // log in redis
                  if let Err(e) = rate_limiter.record_orders(&who, total_objects).await {
                    println!("recording orders to redis db failed with: {:?}", e);
                    break;
                  }
                  // spawn a task to start the ob engine
                  tokio::spawn(process_start_message(tx.clone(), total_objects, mean_price, sd_price, order_probs, best_price_levels));
                },
                WsRequest::Stop => {
                  println!(">>> {} requested STOP", who);
                  graceful_ws_closure(sender, ws::close_code::NORMAL, "client requested to stop simulation").await;
                  break;
                },
                WsRequest::Ack => {
                  println!(">>> {} sent simulation completion ack", who);
                  graceful_ws_closure(sender, ws::close_code::NORMAL, "simulation complete").await;
                  break;
                }
              }
            },
            Message::Close(_c) => {
              println!(">> {} sent CloseFrame msg", who);
              break;
            },
            _ => println!(">> {} sent Binary, Ping or Pong", who)
          }
        }
      }

      Some(msg) = rx.recv() => {
        
        match msg {
          Simulation::Complete => {
            if !batch.is_empty() {
              // Send final batch
              println!("sending rem updates: {:?}", &batch.len());
              let json_data = serde_json::to_string(&batch).expect("serializing final server updates failed!");
              let update_msg = if use_compression {
                let compressed = compress_data(&json_data).map_err(|e| println!("failed to compress final batch with {:?}", e)).expect("Compression Failure!");
                Message::binary(compressed)
              } else {
                Message::text(json_data)
              };

              if sender.send(update_msg).await.is_err() {
                break;
              }
            }
            // no need to compress here, small message
            let completion_signal = WsResponse::Completed;
            let completion_msg = Message::text(serde_json::to_string(&vec![[completion_signal]]).expect("serializing simulation completion signal failed!"));

            if let Err(e) = sender.send(completion_msg).await {
              // return Err(AppError::WebSocketError(format!("Failed to send simulation complete signal to client: {:?}", e)));
              println!("sending simulation completion signal to client failed: {:?}", e);
              break;
            } else {
              println!("Successfully sent completion signal")
            }
          },
          Simulation::Start(snapshot) => {
            // Send intial snapshot to WebSocket Client immediately
            let json_data = serde_json::to_string(&vec![snapshot]).expect("serializing snapshot failed!");
            let snapshot_msg = if use_compression {
              // compress snapshot
              let compressed = compress_data(&json_data).map_err(|e| println!("failed to compress snapshot with {:?}", e)).expect("Compression Failure!");
              Message::binary(compressed)
            } else {
              Message::text(json_data)
            };
            if sender.send(snapshot_msg).await.is_err() {
              break;
            }
          },
          Simulation::Data(data_updates) => {
            // Send batched updates to the Websocket Client 
            batch.push(data_updates);

            if batch.len() >= BATCH_SIZE {
              //println!("batch: {:?}", &batch.len());
              // let batch_size = mem::size_of_val(&*batch);
              // println!("size of messages: {:?}", batch_size);
              let json_data = serde_json::to_string(&batch).expect("serializing batched ws updates failed!");
              let update_msg = if use_compression {
                // Compress the batch data
                let compressed = compress_data(&json_data).map_err(|e| println!("failed to compress ws batch with {:?}", e)).expect("Compression Failure!");
                Message::binary(compressed)
              } else {
                Message::text(json_data)
              };

              if sender.send(update_msg).await.is_err() {
                break;
              }
              batch.clear();
              batch_count += 1;

              if batch_count % BREATHE_AFTER_BATCHES == 0 {
                sleep(Duration::from_millis(BREATHING_TIME_MS)).await;
              }
            }
          }
        }
      }
    }
  }

  println!("Websocket context destroyed for: {}", who);
}

async fn process_start_message(tx: mpsc::Sender<Simulation>, num_orders: usize, mean_price: f64, sd_price: f64, order_probs: Vec<f32>, best_price_lvls: bool) {

  let mut simulator = Simulator::new(mean_price, sd_price, order_probs, best_price_lvls);
  // seed the orderbook with 10k ADD limit orders
  simulator.seed_orderbook(10_000);
  let snapshot = simulator.get_snapshot();
  
  if tx.send(Simulation::Start(snapshot)).await.is_err() {
    panic!("receiver half of channel dropped when sending initial snapshot!");
  }

  println!("[INFO] Starting simulation");
  for idx in 0..num_orders {
    // generate and process the orders
    simulator.generate_orders();
    let updates = simulator.generate_updates(idx);

    if tx.send(Simulation::Data(updates)).await.is_err() {
      panic!("receiver half of channel dropped!");
    };
  }
  //println!("trades: {:?}", simulator.book.executed_orders);
  println!("[INFO] Completed simulation (total trades: {:?})", simulator.book.executed_orders.len());

  if tx.send(Simulation::Complete).await.is_err() { 
    panic!("Could not send close signal to channel after simulation was complete!");
  }
}

// helper function to compress data
fn compress_data(data: &str) -> Result<Vec<u8>, AppError> {
  let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
  encoder.write_all(data.as_bytes()).map_err(|e| AppError::InternalError(e.to_string()))?;
  let compressed_data = encoder.finish().map_err(|e| AppError::InternalError(e.to_string()))?;
  Ok(compressed_data)
}

// helper to close the Websocket gracefully
async fn graceful_ws_closure(mut sender: SplitSink<WebSocket, Message>, code: u16, reason_str: & 'static str) {
  //send a closeframe
  if let Err(e) = sender.send(Message::Close(Some(CloseFrame {
    code,
    reason: Utf8Bytes::from_static(reason_str)
  }))).await {
    println!("error sending close frame: {:?}", e);
  }
  // flush to ensure all messages are sent
  if let Err(e) = sender.flush().await{
    println!("error flushing sender: {:?}", e);
  };
}