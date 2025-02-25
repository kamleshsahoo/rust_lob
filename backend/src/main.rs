mod order_generator;
mod engine;
mod file_upload;

use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::{Duration, Instant}};
use axum::{body::Bytes, extract::{ws::{Message, WebSocket}, ConnectInfo, DefaultBodyLimit, Multipart, State, WebSocketUpgrade}, http::{HeaderValue, Method, StatusCode}, response::IntoResponse, routing::{any, post}, Json, Router};
use futures::lock::Mutex;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::{net::TcpListener, sync::mpsc, time::sleep};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use order_generator::gen::{ServerMessage, Simulator};
use file_upload::{parser::parse_file_orders, upload::{process_uploaded_orders, FileUploadOrderType, FinalStats, LargeUploadResponse, UploadError, UploadRequest, UploadResponse}};

type OrderSender = mpsc::Sender<Vec<FileUploadOrderType>>;
type OrderReceiver = mpsc::Receiver<Vec<FileUploadOrderType>>;
type ResultSender = mpsc::Sender<HashMap<String, FinalStats>>;
type ResultReceiver = mpsc::Receiver<HashMap<String, FinalStats>>;
// allow max file uploads of 300MB for the /largeupload route
const MAX_FILE_SIZE: usize = 1024 * 1024 * 300;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
  Start { 
      // client_name: String, 
      total_objects: usize,  // Optional, defaults to 50_000
      throttle_nanos: u64, // Optional, defaults to 1000ns
      mean_price: f64,  // Optional, defaults to 300.0
      sd_price: f64,  // Optional, defaults to 50.0
      best_price_levels: bool // whether to show best bids and asks, defaults to false
  },
  Stop,
}

enum Simulation {
  Start(Vec<ServerMessage>),
  Data(Vec<ServerMessage>),
  Complete
}

struct FileUploadSessionData {
  order_sender: OrderSender,
  result_receiver: ResultReceiver,
  created_at: Instant,
  total_chunks: usize,
  total_orders: usize,
  processed_chunks: usize
}

#[derive(Clone)]
struct UploadSessionManager {
  sessions: Arc<Mutex<HashMap<String, FileUploadSessionData>>>
}

impl UploadSessionManager {
  fn new() -> Self {
    Self { sessions: Arc::new(Mutex::new(HashMap::new())) }
  }

  async fn create_session(&self, session_id: String, total_chunks: usize, total_orders: usize) -> OrderSender {
    let (order_tx, order_rx) = mpsc::channel(1_000);
    let (result_tx, result_rx) = mpsc::channel(1_000);

    let session = FileUploadSessionData {
      order_sender: order_tx.clone(),
      result_receiver: result_rx,
      created_at: Instant::now(),
      total_chunks,
      total_orders,
      processed_chunks: 0
    };

    let mut sessions = self.sessions.lock().await;
    sessions.insert(session_id, session);

    Self::spawn_processing_task(order_rx, result_tx, total_orders);

    // (order_rx, result_tx)
    order_tx
  }

  fn spawn_processing_task(mut order_rx: OrderReceiver, result_tx: ResultSender, total_orders: usize) {
    tokio::spawn(async move {
      let mut all_orders = Vec::new();
        while let Some(chunk) = order_rx.recv().await {
          all_orders.extend(chunk);
          // process when all orders are recvd
          if all_orders.len() >= total_orders {
            let result = process_uploaded_orders(all_orders).await;
            result_tx.send(result).await.expect("failed to send final result on channel!");
            break;
          }
        }
    });
  }

  async fn get_chunk_sender(&self, session_id: &str) -> Option<OrderSender> {
    let sessions = self.sessions.lock().await;
    sessions.get(session_id).map(|s| s.order_sender.clone())
  } 

  async fn take_result_receiver(&self, session_id: &str) -> Option<ResultReceiver> {
    let mut sessions = self.sessions.lock().await;
    sessions.remove(session_id).map(|s| s.result_receiver)
  }
  /* TODO: Cleanup old sessions */
} 


#[tokio::main]
async fn main() {

  let session_manager = UploadSessionManager::new();

  let cors = CorsLayer::new()
  .allow_methods([Method::GET, Method::POST])
  .allow_origin("http://127.0.0.1:8080".parse::<HeaderValue>().unwrap());
  // .allow_headers([CONTENT_TYPE]);
    
  let app = Router::new()
  .route("/wslob", any(ws_handler))
  .route("/smallupload", post(small_upload_handler).with_state(session_manager))
  .route("/largeupload", post(large_upload_handler).layer(DefaultBodyLimit::max(MAX_FILE_SIZE)))
  // .with_state(session_manager)
  .layer(cors);

  let listener = TcpListener::bind("0.0.0.0:7575").await.expect("failed to start tcp listener");

  axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.expect("failed to start server");
}

async fn ws_handler (ws: WebSocketUpgrade, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
  // TODO: maybe add UserAgent info!
  println!("User at {} connected.", addr);
  ws.on_upgrade(move|socket| handle_socket(socket, addr))
}

async fn handle_socket(socket: WebSocket, who: SocketAddr) {

  let (mut sender, mut receiver) = socket.split();
  let (tx, mut rx) = mpsc::channel(1_000_000);

  let mut batch: Vec<Vec<ServerMessage>>  = vec![];
  let mut batch_count: usize = 0;
  const BATCH_SIZE: usize = 500;
  const BREATHE_AFTER_BATCHES: usize = 10;
  const BREATHING_TIME_MS: u64 = 1;

  loop { 
    tokio::select! {
      msg = receiver.next() => {
        if let Some(Ok(client_msg)) = msg {
          match client_msg {
            Message::Text(t) => {
              println!(">>> {} sent string: {:?}", who, t);
              let payload = serde_json::from_str::<ClientMessage>(t.as_str()).expect("derserializng client message failed");
              match payload {
                ClientMessage::Start {total_objects, throttle_nanos, mean_price, sd_price , best_price_levels} => {
                  println!("client payload\ntotal objects: {:?} nanos: {:?} mean: {:?} sd: {:?} show best price levels: {:?}", total_objects, throttle_nanos, mean_price, sd_price, best_price_levels);
                  // spawn a task 
                  tokio::spawn(process_start_message(tx.clone(), total_objects, mean_price, sd_price, best_price_levels, throttle_nanos));
               },
               ClientMessage::Stop => {
                println!(">>> {} requested STOP", who);
                

                break;
               }
              }
            },
            Message::Close(_c) => {
              println!(">> {} sent CloseFrame msg", who);
              break;
            },
            _ => {
              println!(">> {} sent Binary, Ping or Pong", who);
            }
          }
        }
      }

      Some(msg) = rx.recv() => {
        
        match msg {
          Simulation::Complete => {
            println!("Simulation complete. Closing the simulation channel and droping the socket connection");
            if !batch.is_empty() {
              println!("sending rem updates: {:?}", &batch.len());
              let update_msg = Message::text(serde_json::to_string(&batch).expect("serializing final server updates failed!"));
              if sender.send(update_msg).await.is_err() {
                break;
              }
            }
            rx.close();

            let completion_signal = ServerMessage::Completed;
            let completion_msg = Message::text(serde_json::to_string(&vec![[completion_signal]]).expect("serializing simulation completion signal failed!"));
            if sender.send(completion_msg).await.is_err() {
              println!("sending simulation completion signal to dioxus failed!");
            }
            break;
          },
          Simulation::Start(snapshot) => {
            // << Send intial snapshot to WebSocket Client immediately >>
            let snapshot_msg = Message::text(serde_json::to_string(&vec![snapshot]).expect("serializing snapshot failed!"));
            if sender.send(snapshot_msg).await.is_err() {
              break;
            }
          },
          Simulation::Data(data_updates) => {
            //<< Send batched updates to the Websocket Client >>
            batch.push(data_updates);

            if batch.len() >= BATCH_SIZE {
              //println!("batch: {:?}", &batch.len());
              // let batch_size = mem::size_of_val(&*batch);
              // println!("size of messages: {:?}", batch_size);
              let update_msg = Message::text(serde_json::to_string(&batch).expect("serializing server updates failed!"));
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

  println!("Websocket context {} destroyed", who);
}

async fn process_start_message(tx: mpsc::Sender<Simulation>, num_orders: usize, mean_price: f64, sd_price: f64, best_price_lvls: bool, throttle: u64) {
  //TODO: see if we need to add throttling
  let mut simulator = Simulator::new(mean_price, sd_price, best_price_lvls);
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
  println!("[INFO] Completed simulation (total trades: {:?}), sending close frame to channel", simulator.book.executed_orders.len());

  if tx.send(Simulation::Complete).await.is_err() { 
    panic!("Could not send close signal to channel after simulation was complete!");
  }
}

async fn small_upload_handler(State(session_manager): State<UploadSessionManager>, body: Bytes) -> Result<Json<UploadResponse>, UploadError> {
  
  //println!("User uploaded file.");
  let payload = match <UploadRequest>::deserialize(&mut rmp_serde::Deserializer::new(&body[..])) {
    Ok(de_payload) => de_payload,
    Err(e) => {
      println!("couldnot deserialize the client file orders: {:?}", e);
      return Err(UploadError::DeserializeError(e.to_string()))
    }
  };

  let session_id = match payload.session_id {
    Some(id) => id,
    None => Uuid::new_v4().to_string()
  };
  
  match payload.chunk_number {
    0 => {
      let order_tx = session_manager.create_session(session_id.clone(), payload.total_chunks, payload.total_orders).await;
      // let total_orders = payload.total_orders;
      if let Err(e) = order_tx.send(payload.orders).await {
        println!("sending 1st chunk on channel failed!");
        return Err(UploadError::ChannelError(e.to_string()));
      }

      // check if we have only one chunk
      if payload.total_chunks != 1 {
        return Ok(Json(
          UploadResponse {
            data: None,
            session_id: Some(session_id),
          }
        ));
      } else {
        match session_manager.take_result_receiver(&session_id).await {
          Some(mut rcvr) => {
            match rcvr.recv().await {
              Some(res) => {
                return Ok(Json(
                  UploadResponse {
                    data: Some(res),
                    session_id: Some(session_id),
                  }
                ));
              },
              None => return Err(UploadError::ChannelError("sender dropped when receiving the final result".to_string()))
            }
          },
          None => return Err(UploadError::SessionNotFound)
        };
      }
    },
    n if n == payload.total_chunks - 1 => {
      // last chunk
      let order_sender = session_manager.get_chunk_sender(&session_id).await;
      let result_receiver = session_manager.take_result_receiver(&session_id).await;
      match (order_sender, result_receiver) {
        (Some(sender), Some(mut rcvr)) => {
          // send the final chunk
          if let Err(e) = sender.send(payload.orders).await {
            println!("the mpsc orders receiver dropped");
            return Err(UploadError::ChannelError(e.to_string()));
          }

          match rcvr.recv().await {
            Some(res) => {
              return Ok(Json(
                UploadResponse {
                  data: Some(res),
                  session_id: Some(session_id),
                }
              ));
            },
            None => return Err(UploadError::ChannelError("sender dropped when receiving the final result".to_string()))
          }
        },
        _ => return Err(UploadError::SessionNotFound),
      } 
    },
    n if n < payload.total_chunks - 1 => {
      // for subsequent chunks
      match session_manager.get_chunk_sender(&session_id).await {
          Some(order_sender) => {
            if let Err(e) = order_sender.send(payload.orders).await {
              println!("the mpsc orders receiver dropped");
              return Err(UploadError::ChannelError(e.to_string()));
            };
            return Ok(
              Json(
                UploadResponse {
                  data: None,
                  session_id: Some(session_id),
                }
              ));
          },
          None => {
            return Err(UploadError::SessionNotFound);
          }
      }  
    },
    _ => return Err(UploadError::InvalidChunk)
  }
}

async fn large_upload_handler(mut multipart: Multipart) -> Result<Json<LargeUploadResponse>, (StatusCode, String)> {

  let mut result = None;
  let mut parse_duration = Duration::default();
  let mut total_raw_orders = 0;
  let mut invalid_orders = 0;

  if let Some(field) = multipart.next_field().await.map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))? {
    let file_contents = field.text().await.map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

    let (parsed_orders, duration, raw_cnt, invalid_cnt) = parse_file_orders(file_contents);
    parse_duration = duration;
    total_raw_orders = raw_cnt;
    invalid_orders = invalid_cnt;
    result = Some(process_uploaded_orders(parsed_orders).await);
  } else {
    return Err((StatusCode::BAD_REQUEST, "No file found".to_string()));
  }

  let orderbook_results = result.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to process orders".to_string()))?;
  
  Ok(Json(LargeUploadResponse {
    orderbook_results,
    parse_results: (parse_duration, total_raw_orders, invalid_orders)
  }))
}
  
/*
while let Some(field) = multipart.next_field().await.unwrap() {
  let name = field.name().unwrap().to_string();
  let file_contents = field.text().await.unwrap();
  //let data_bytes = field.bytes().await.unwrap();
  // println!("uploaded data `{:?}` has textized data: {:?}", name, &data_txt);
  // println!("Length of `{}` is {} bytes", name, data_bytes.len());
  let (parsed_orders, parse_duration, total_raw_orders, invalid_orders) = parse_file_orders(file_contents);

  let result = process_uploaded_orders(parsed_orders).await;

  return ;
}
*/