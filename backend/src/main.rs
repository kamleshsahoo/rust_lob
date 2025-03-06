mod order_generator;
mod engine;
mod file_upload;
// mod cloud_auth;

use std::{collections::HashMap, env, net::SocketAddr, sync::Arc, time::{Duration, Instant}};
use axum::{body::Bytes, extract::{ws::{Message, WebSocket}, ConnectInfo, DefaultBodyLimit, Multipart, State, WebSocketUpgrade}, http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode}, response::IntoResponse, routing::{any, get, post}, Json, Router};
// use cloud_auth::auth::{get_signed_url, MySignedURLOptions};
use futures::lock::Mutex;
use futures_util::{SinkExt, StreamExt};

// use ::google_cloud_auth::credentials::create_access_token_credential;
// use google_cloud_storage::{client::{Client, ClientConfig}, http::objects::{download::Range, get::GetObjectRequest}, sign::{self, SignedURLMethod, SignedURLOptions}};
// use google_cloud_token::TokenSourceProvider;
// use google_cloud_auth::project::Config;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::{net::TcpListener, sync::mpsc, time::sleep};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use order_generator::gen::{ServerMessage, Simulator};
use file_upload::{parser::parse_file_orders, upload::{process_uploaded_orders, FileUploadOrderType, FinalStats, LargeUploadRequest, LargeUploadResponse, LargeUploadSessionManager, UploadError, UploadRequest, UploadResponse}};

type OrderSender = mpsc::Sender<Vec<FileUploadOrderType>>;
type OrderReceiver = mpsc::Receiver<Vec<FileUploadOrderType>>;
type ResultSender = mpsc::Sender<HashMap<String, FinalStats>>;
type ResultReceiver = mpsc::Receiver<HashMap<String, FinalStats>>;
// allow max file uploads of 10MB for the /largeupload route
const MAX_FILE_SIZE: usize = 1024 * 1024 * 30;

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
  Ack
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

#[derive(Deserialize)]
struct GenerateSignedUrlRequest {
    file_name: String,
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
            result_tx.send(result).await.expect("failed to send final result for smallfile on channel!");
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

  let small_upload_session_manager = UploadSessionManager::new();
  let large_upload_session_manager = LargeUploadSessionManager::new();

  let cors = CorsLayer::new()
  .allow_methods([Method::GET, Method::POST])
  //.allow_origin("http://127.0.0.1:8080".parse::<HeaderValue>().unwrap())
  .allow_origin(Any)
  .allow_headers([CONTENT_TYPE]);
    
  let app = Router::new()
  .route("/health", get(health_check_handler))
  // .route("/generate-url", post(generate_signed_url))
  // .route("/process-file", post(process_file))
  .route("/wslob", any(ws_handler))
  .route("/smallupload", post(small_upload_handler)
                        .with_state(small_upload_session_manager)
        )
  .route("/largeupload", post(large_upload_handler_v2)
                        .layer(DefaultBodyLimit::max(MAX_FILE_SIZE))
                        .with_state(large_upload_session_manager)
        )
  // .with_state(session_manager)
  .layer(cors);

  let listener = TcpListener::bind("0.0.0.0:7575").await.expect("failed to start tcp listener");

  axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.expect("failed to start server");
}

async fn health_check_handler() -> Json<serde_json::Value> {
  Json(json!({"code":200, "status": "healthy"}))
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
               },
               ClientMessage::Ack => {
                println!(">>> {} sent simulation completion ack", who);
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
            if !batch.is_empty() {
              println!("sending rem updates: {:?}", &batch.len());
              let update_msg = Message::text(serde_json::to_string(&batch).expect("serializing final server updates failed!"));
              if sender.send(update_msg).await.is_err() {
                break;
              }
            }
  
            let completion_signal = ServerMessage::Completed;
            let completion_msg = Message::text(serde_json::to_string(&vec![[completion_signal]]).expect("serializing simulation completion signal failed!"));

            if let Err(e) = sender.send(completion_msg).await {
              println!("sending simulation completion signal to dioxus failed: {:?}", e);
              break;
            } else {
              println!("Successfully sent completion signal")
            }

            //tokio::time::sleep(Duration::from_millis(100)).await;

            // if sender.close().await.is_err() {
            //   println!("closing connection failed!")
            // }
            // rx.close();
            // break;
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
  println!("[INFO] Completed simulation (total trades: {:?})", simulator.book.executed_orders.len());

  if tx.send(Simulation::Complete).await.is_err() { 
    panic!("Could not send close signal to channel after simulation was complete!");
  }
}

async fn small_upload_handler(State(session_manager): State<UploadSessionManager>, body: Bytes) -> Result<Json<UploadResponse>, UploadError> {
  
  //println!("User uploaded file.");
  let payload = match <UploadRequest>::deserialize(&mut rmp_serde::Deserializer::new(&body[..])) {
    Ok(de_payload) => de_payload,
    Err(e) => {
      println!("couldnot deserialize the small file upload request: {:?}", e);
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
        println!("sending 1st small file orders chunk on channel failed!");
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
              None => return Err(UploadError::ChannelError("sender dropped somehow when receiving the final result for small file uplaod".to_string()))
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
          // send the final chunk on channel
          if let Err(e) = sender.send(payload.orders).await {
            println!("the receiver channel somehow dropped when sending the final chunk for small upload!");
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
            None => return Err(UploadError::ChannelError("sender channel somehow dropped when receiving the final result for small file upload".to_string()))
          }
        },
        _ => return Err(UploadError::SessionNotFound),
      } 
    },
    n if n < payload.total_chunks - 1 => {
      // for all other chunks
      match session_manager.get_chunk_sender(&session_id).await {
        Some(order_sender) => {
          if let Err(e) = order_sender.send(payload.orders).await {
            println!("small file upload receiver channel somehow dropped");
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
          None => return Err(UploadError::SessionNotFound)
        }
      },
    _ => return Err(UploadError::InvalidChunk)
  }
}

/* Working Ver - To be Deprecated
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
*/

async fn large_upload_handler_v2 (
  State(session_manager): State<LargeUploadSessionManager>,
  body: Bytes
) -> Result<Json<LargeUploadResponse>, UploadError> {
  
  let payload = match <LargeUploadRequest>::deserialize(&mut rmp_serde::Deserializer::new(&body[..])) {
    Ok(de_payload) => de_payload,
    Err(e) => {
      println!("couldnot deserialize the large file upload request: {:?}", e);
      return Err(UploadError::DeserializeError(e.to_string()));
    }
  };

  let session_id = match payload.session_id {
    Some(id) => id,
    None => Uuid::new_v4().to_string()
  };

  match payload.chunk_number {
    0 => {
      let chunk_tx = session_manager.create_session(session_id.clone(), payload.total_chunks).await;
      if let Err(e) = chunk_tx.send(payload.chunk).await {
        println!("sending 1st large file chunk on channel failed!");
        return Err(UploadError::ChannelError(e.to_string()));     
      }

      // check for one and only chunk
      if payload.total_chunks != 1 {
        return Ok(Json(
          LargeUploadResponse {
            orderbook_results: None,
            parse_results: None,
            session_id: Some(session_id)
          }
        ));
      } else {
        match session_manager.take_result_receiver(&session_id).await {
          Some(mut rcvr) => {
            match rcvr.recv().await {
              Some(res) => {
                return Ok(Json(
                  LargeUploadResponse {
                    orderbook_results: Some(res.0),
                    parse_results: Some(res.1),
                    session_id: Some(session_id) }
                ));
              },
              None => return Err(UploadError::ChannelError("sender dropped somehow when receiving the final result for large file upload".to_string()))
            }
          },
          None => return Err(UploadError::SessionNotFound)
        };
      }
    },
    n if n == payload.total_chunks - 1 => {
      // last chunk
      let chunk_sender = session_manager.get_chunk_sender(&session_id).await;
      let result_receiver = session_manager.take_result_receiver(&session_id).await;

      match (chunk_sender, result_receiver) {
        (Some(sender), Some(mut rcvr)) => {
          // send final chunk on channel
          if let Err(e) = sender.send(payload.chunk).await {
            println!("the receiver channel somehow dropped when sending the final chunk for large upload!");
            return Err(UploadError::ChannelError(e.to_string()));
          }

          match rcvr.recv().await {
            Some(res) => {
              return Ok(Json(
                LargeUploadResponse {
                  orderbook_results: Some(res.0),
                  parse_results: Some(res.1),
                  session_id: Some(session_id)
                }
              ));
            },
            None => return Err(UploadError::ChannelError("sender channel somehow dropped when receiving the final result for large file upload".to_string()))
          }
        },
        _ => return Err(UploadError::SessionNotFound)
      }
    },
    n if n < payload.total_chunks - 1 => {
      // for all other chunks
      match session_manager.get_chunk_sender(&session_id).await {
        Some(chunk_sender) => {
          if let Err(e) = chunk_sender.send(payload.chunk).await {
            println!("large file upload receiver channel somehow dropped");
            return Err(UploadError::ChannelError(e.to_string()));
          };
          return Ok(Json(
            LargeUploadResponse {
              orderbook_results: None,
              parse_results: None,
              session_id: Some(session_id) }
          ));
        },
        None => return Err(UploadError::SessionNotFound)
      }
    },
    _ => return Err(UploadError::InvalidChunk)
  }
}


/*GCP signBlob api
async fn generate_signed_url(Json(payload): Json<GenerateSignedUrlRequest>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {

  // let creds_file_path = env::var("GOOG_SA_CREDS").map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, "sa account credentials file environment variable not set".to_string()))?;
  // println!("creds file path: {}", &creds_file_path);

  // let credentials_json = fs::read(creds_file_path).expect("failed to read creds json");
  // let credentials_file = serde_json::from_slice::<CredentialsFile>(&credentials_json).expect("error deserializing creds from json!");

  // let tsp = DefaultTokenSourceProvider::new_with_credentials(
  //   Config::default().with_scopes(&["https://www.googleapis.com/auth/cloud-platform"]),
  //   Box::new(credentials_file)
  // ).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  let object_name = payload.file_name;
  let google_access_id = env::var("GAI").map_err(|_e| (StatusCode::INTERNAL_SERVER_ERROR, "could not find sa account email from environment variable not set".to_string()))?;
  println!("sa account email: {}", &google_access_id);

  let config = ClientConfig::default().with_auth().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  println!("client config: {:?}", config);
  
  // let tsp = config.token_source_provider.expect("should have a tsp for authorized client"); 
  // let ts = tsp.token_source();
  // let token = ts.token().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  // println!("[**]Token: {}", &token);
 
  // let signed_url = get_signed_url("lob-app-bucket", 
  //   &object_name,
  //   google_access_id,
  //   ts,
  //   MySignedURLOptions {method: SignedURLMethod::PUT, expires: Duration::from_secs(300)}
  // )
  // .await
  // .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

  let client = Client::new(config);
  // let bucket_name = format!("{}_gcrgcs_{}", project, name);
  let options = SignedURLOptions {
    method: SignedURLMethod::PUT,
    expires: Duration::from_secs(600),
    ..SignedURLOptions::default()
  };
  let signed_url = client.signed_url(
    "lob-app-bucket",
    &object_name,
    None,
    None,
    options)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

  println!("signed url: {}", &signed_url);

  // println!("sending signed url: {}", &url_for_upload);

  Ok(Json(json!({"signed_url": signed_url})))
}

async fn process_file(Json(payload): Json<GenerateSignedUrlRequest>) -> Result<Json<LargeUploadResponse>, (StatusCode, String)> {
  let config = ClientConfig::default().with_auth().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
  let client = Client::new(config);
  println!("[INFO]gclient config success for signed url!");
  let object_name = payload.file_name;

  let result = client.download_object(
    &GetObjectRequest {
      bucket: "lob-app-bucket".to_string(),
      object: object_name,
      ..Default::default()
    }, &Range::default()).await.map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

  let file_contents = String::from_utf8(result).map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

  let (parsed_orders, duration, raw_cnt, invalid_cnt) = parse_file_orders(file_contents);
    // parse_duration = duration;
    // total_raw_orders = raw_cnt;8+
    // invalid_orders = invalid_cnt;
  let results = process_uploaded_orders(parsed_orders).await;

  Ok(Json(LargeUploadResponse {
    orderbook_results: results,
    parse_results: (duration, raw_cnt, invalid_cnt)
  }))

}
*/