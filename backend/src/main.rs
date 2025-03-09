mod order_generator;
mod engine;
mod file_upload;
mod midwares;

use std::{collections::HashMap, env, net::SocketAddr, sync::Arc, time::{Duration, Instant}};
use axum::{
  body::Bytes,
  extract::{ws::{Message, WebSocket}, DefaultBodyLimit, Multipart, State, WebSocketUpgrade},
  http::{header::CONTENT_TYPE, Method},
  middleware,
  response::IntoResponse,
  routing::{any, get, post},
  Extension,
  Json,
  Router
};
use futures::lock::Mutex;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::{net::TcpListener, sync::mpsc, time::sleep};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use order_generator::gen::{WsResponse, Simulator};
use file_upload::{parser::parse_file_orders_v2, upload::{process_uploaded_orders, FileUploadOrderType, FinalStats, LargeUploadResponse, LargeUploadSessionManager, UploadRequest, UploadResponse}};
use midwares::app_state::{estimate_orders_from_1stchunk, ip_tracker, AppError, PostgresDBPool, RateLimiter, RequestContext};

type OrderSender = mpsc::Sender<Vec<FileUploadOrderType>>;
type OrderReceiver = mpsc::Receiver<Vec<FileUploadOrderType>>;
type ResultSender = mpsc::Sender<HashMap<String, FinalStats>>;
type ResultReceiver = mpsc::Receiver<HashMap<String, FinalStats>>;
// allow max file uploads of 15MB for the /largeupload route
const MAX_FILE_SIZE: usize = 1024 * 1024 * 15;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum WsRequest {
  Start {
      total_objects: usize,  // Optional, defaults to 50_000
      // throttle_nanos: u64, // Optional, defaults to 1000ns
      mean_price: f64,  // Optional, defaults to 300.0
      sd_price: f64,  // Optional, defaults to 50.0
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

struct FileUploadSessionData {
  order_sender: OrderSender,
  result_receiver: ResultReceiver,
  created_at: Instant,
  total_chunks: usize,
  total_orders: usize,
  processed_chunks: usize
}

// #[derive(Deserialize)]
// struct GenerateSignedUrlRequest {
//     file_name: String,
// }

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
            let result = process_uploaded_orders(all_orders);
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

  
  
  const REDIS_URL: &str = "xxxx";
  const DB_URL: &str = "xxxx";

  let rate_limiter = RateLimiter::new(REDIS_URL).expect("failed to create ratelimiterl!");
  let db_pool = PostgresDBPool::new(DB_URL).await.expect("failed to create postgres connection pool!");

  let small_upload_session_manager = UploadSessionManager::new();
  let large_upload_session_manager = LargeUploadSessionManager::new();

  let cors = CorsLayer::new()
  .allow_methods([Method::GET, Method::POST])
  //.allow_origin("http://127.0.0.1:8080".parse::<HeaderValue>().unwrap())
  .allow_origin(Any)
  .allow_headers([CONTENT_TYPE]);
    
  /*depc.
  let app = Router::new()
  .route("/health", get(health_check_handler))
  .route("/wslob", any(ws_handler))
  .route("/smallupload", post(small_upload_handler)
                        .with_state(small_upload_session_manager)
        )
  .route("/largeupload", post(large_upload_handler)
                        .layer(DefaultBodyLimit::max(MAX_FILE_SIZE))
                        .with_state(large_upload_session_manager)
        )
  .layer(Extension(rate_limiter))
  .layer(Extension(db_pool))
  .layer(middleware::from_fn(ip_tracker))
  .layer(cors);
  */

  let with_middleware = Router::new()
    .route("/wslob", any(ws_handler))
    .route("/smallupload", post(small_upload_handler)
              .with_state(small_upload_session_manager))
    .route("/largeupload", post(large_upload_handler)
            .layer(DefaultBodyLimit::max(MAX_FILE_SIZE))
            .with_state(large_upload_session_manager))
    .layer(Extension(rate_limiter))
    .layer(middleware::from_fn(ip_tracker))
    .layer(Extension(db_pool));

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

async fn ws_handler (
  ws: WebSocketUpgrade,
  //user_agent: Option<TypedHeader<headers>>,
  // ConnectInfo(addr): ConnectInfo<SocketAddr>,
  Extension(rate_limiter): Extension<RateLimiter>,
  Extension(ctx): Extension<RequestContext>,
) -> impl IntoResponse {
  // TODO: maybe add UserAgent info!
  // println!("User at {} connected.", addr);
  ws.on_upgrade(move|socket| handle_socket(socket, ctx, rate_limiter))
}

async fn handle_socket(
  socket: WebSocket,
  // who: SocketAddr,
  ctx: RequestContext,
  rate_limiter: RateLimiter
) {

  let (mut sender, mut receiver) = socket.split();
  let (tx, mut rx) = mpsc::channel(1_000_000);
  let who = ctx.remote_ip;

  let mut batch: Vec<Vec<WsResponse>>  = vec![];
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

              let payload = serde_json::from_str::<WsRequest>(t.as_str()).expect("derserializng client message failed");      
            
              match payload {
                WsRequest::Start {total_objects, mean_price, sd_price , best_price_levels} => {
                  println!("client payload\ntotal orders: {:?} mean: {:?} sd: {:?} show best price levels: {:?}", total_objects, mean_price, sd_price, best_price_levels);
                  if let Err(e) = rate_limiter.would_exceed_limit(&who, &total_objects).await {
                    println!("{:?}", e);
                    // send a msg to client and close the connection
                    let rl_exceeded_signal = WsResponse::RateLimitExceeded;
                    let rl_exceeded_msg = Message::text(serde_json::to_string(&vec![[rl_exceeded_signal]]).expect("serializing rate limit exceeded signal failed!"));
                    if let Err(e) = sender.send(rl_exceeded_msg).await {
                      println!("sending rate limit exceeded message to client failed with: {:?}", e);
                      break;
                    }
                    // we exit eventually after rate limit was exceeded
                    break;
                  }
                  if let Err(e) = rate_limiter.record_orders(&who, total_objects).await {
                    println!("recording orders to redis db failed with: {:?}", e);
                    break;
                  }
                  // spawn a task to start the ob engine
                  tokio::spawn(process_start_message(tx.clone(), total_objects, mean_price, sd_price, best_price_levels));
                },
                WsRequest::Stop => {
                  println!(">>> {} requested STOP", who);
                  break;
                },
                WsRequest::Ack => {
                  println!(">>> {} sent simulation completion ack", who);
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
              println!("sending rem updates: {:?}", &batch.len());
              let update_msg = Message::text(serde_json::to_string(&batch).expect("serializing final server updates failed!"));
              if sender.send(update_msg).await.is_err() {
                break;
              }
            }
  
            let completion_signal = WsResponse::Completed;
            let completion_msg = Message::text(serde_json::to_string(&vec![[completion_signal]]).expect("serializing simulation completion signal failed!"));

            if let Err(e) = sender.send(completion_msg).await {
              // return Err(AppError::WebSocketError(format!("Failed to send simulation complete signal to client: {:?}", e)));
              println!("sending simulation completion signal to client failed: {:?}", e);
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

  println!("Websocket context destroyed for: {}", who);
}

async fn process_start_message(tx: mpsc::Sender<Simulation>, num_orders: usize, mean_price: f64, sd_price: f64, best_price_lvls: bool) {
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

async fn small_upload_handler(
  State(session_manager): State<UploadSessionManager>,
  Extension(rate_limiter): Extension<RateLimiter>,
  Extension(req_ctx): Extension<RequestContext>,
  body: Bytes
) -> Result<Json<UploadResponse>, AppError> {
  
  //println!("User uploaded file.");
  let payload = match <UploadRequest>::deserialize(&mut rmp_serde::Deserializer::new(&body[..])) {
    Ok(de_payload) => de_payload,
    Err(e) => {
      println!("couldnot deserialize the small file upload request: {:?}", e);
      return Err(AppError::DeserializeError(e.to_string()))
    }
  };

  let remote_ip = req_ctx.remote_ip;
  let total_orders = payload.total_orders;

  rate_limiter.would_exceed_limit(&remote_ip, &total_orders).await?;
  rate_limiter.record_orders(&remote_ip, total_orders).await?;

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
        return Err(AppError::InternalError(e.to_string()));
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
              None => return Err(AppError::InternalError("sender dropped somehow when receiving the final result for small file uplaod".to_string()))
            }
          },
          None => return Err(AppError::InternalError("small file upload session not found".to_owned()))
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
            return Err(AppError::InternalError(e.to_string()));
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
            None => return Err(AppError::InternalError("sender channel somehow dropped when receiving the final result for small file upload".to_string()))
          }
        },
        _ => return Err(AppError::InternalError("small file upload session not found".to_owned())),
      } 
    },
    n if n < payload.total_chunks - 1 => {
      // for all other chunks
      match session_manager.get_chunk_sender(&session_id).await {
        Some(order_sender) => {
          if let Err(e) = order_sender.send(payload.orders).await {
            println!("small file upload receiver channel somehow dropped");
            return Err(AppError::InternalError(e.to_string()));
          };
          return Ok(
            Json(
              UploadResponse {
                data: None,
                session_id: Some(session_id),
              }
            ));
          },
          None => return Err(AppError::InternalError("small file upload session not found".to_owned()))
        }
      },
    _ => return Err(AppError::InternalError("invalid chunk for small file upload".to_owned()))
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

/*Failed Try with simple deserialization, gives OOM
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
*/

async fn large_upload_handler (
  State(state): State<LargeUploadSessionManager>,
  Extension(rate_limiter): Extension<RateLimiter>,
  Extension(req_ctx): Extension<RequestContext>,
  mut multipart: Multipart
) -> Result<Json<LargeUploadResponse>, AppError> {

  // Extract multipart fields
  let mut session_id = None;
  let mut total_chunks = None;
  let mut chunk_number = None;
  let mut chunk_data = None;

  while let Some(field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
    match field.name() {
      Some("session_id") => session_id = Some(field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?),
      Some("total_chunks") => total_chunks = Some(field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?.parse::<usize>().map_err(|_| AppError::BadRequest("Invalid total_chunks value".to_string()))?),
      Some("chunk_number") => chunk_number = Some(field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?.parse::<usize>().map_err(|_| AppError::BadRequest("Invalid chunk_number value".to_string()))?),
      Some("chunk") => chunk_data = Some(field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?),
      _ => {}
    }
  }

  // Validate we got all required fields
  let session_id = session_id.ok_or(AppError::BadRequest("Missing session_id".to_string()))?;
  let total_chunks = total_chunks.ok_or(AppError::BadRequest("Missing total_chunks".to_string()))?;
  let chunk_number = chunk_number.ok_or(AppError::BadRequest("Missing chunk_number".to_string()))?;
  let chunk_data = chunk_data.ok_or(AppError::BadRequest("Missing chunk_data".to_string()))?;

  
  let remote_ip = req_ctx.remote_ip;
  // try to estimate total orders with 1st chunk and return early if too much orders
  if chunk_number == 0 {
    let estimated_orders = estimate_orders_from_1stchunk(&chunk_data, &total_chunks);
    rate_limiter.would_exceed_limit(&remote_ip, &estimated_orders).await?;
  }
  
  state.store_chunk(&session_id, chunk_number, chunk_data, total_chunks).await;

  let is_complete = state.is_upload_complete(&session_id).await;

  if is_complete {
    // get complete file data
    let complete_data = state.get_all_chunks(&session_id).await.map_err(|e| AppError::InternalError(e))?;

    let (parsed_orders, duration, raw_cnt, invalid_cnt) = parse_file_orders_v2(&complete_data);

    // now we check and record actual orders in the ratelimiter
    let total_orders = parsed_orders.len();
    rate_limiter.would_exceed_limit(&remote_ip, &total_orders).await?;
    rate_limiter.record_orders(&remote_ip, total_orders).await?;

    let ob_results = process_uploaded_orders(parsed_orders);

    // Clean up the chunks after processing
    state.clear_chunks(&session_id).await.map_err(|e| AppError::InternalError(e))?;

    return Ok(Json(
      LargeUploadResponse {
        orderbook_results: Some(ob_results),
        parse_results: Some((duration, raw_cnt, invalid_cnt)),
        processed: true
      }
    ));
  }

  // For intermediate chunks, just return acknowledgment
  Ok(Json(
    LargeUploadResponse {
      orderbook_results: None,
      parse_results: None,
      processed: false
    }))

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