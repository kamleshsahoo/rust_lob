mod order_generator;
mod engine;

use std::net::SocketAddr;
use axum::{extract::{ws::{Message, WebSocket}, ConnectInfo, WebSocketUpgrade}, response::IntoResponse, routing::any, Router};
use futures_util::{SinkExt, StreamExt};
use order_generator::gen::{ServerMessage, Simulator};
use serde::Deserialize;
use tokio::{net::TcpListener, sync::mpsc::{self, Sender}};


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

/*
struct WebSocketBatcher {
  last_update: Instant,
  batch_interval: Duration,
  engine_stats: Vec<EngineStats>,
  trades: Vec<ExecutedOrders>
}

impl WebSocketBatcher {
  fn new(socket_batch_interval: Duration) -> Self {
    Self {
      last_update: Instant::now(),
      batch_interval: socket_batch_interval,
      engine_stats: Vec::new(),
      trades: Vec::new(),
    }
  }

  fn should_send(&self) -> bool {
    self.last_update.elapsed() >= self.batch_interval
  }

  fn reset_timer(&mut self) {
    self.last_update = Instant::now();
  }

}
*/

#[tokio::main]
async fn main() {
    
  let app = Router::new().route("/wslob", any(handler));

  let listener = TcpListener::bind("0.0.0.0:7575").await.expect("failed to start tcp listener");

  axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.expect("failed to start server");

}

async fn handler (ws: WebSocketUpgrade, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
  // TODO: maybe add UserAgent info!
  println!("User at {} connected.", addr);
  ws.on_upgrade(move|socket| handle_socket(socket, addr))
}

async fn handle_socket(socket: WebSocket, who: SocketAddr) {

  let (mut sender, mut receiver) = socket.split();
  let (tx, mut rx) = mpsc::channel(1_000_000);

  // let mut ws_batcher = WebSocketBatcher::new(Duration::from_micros(10));
  let mut batch: Vec<Vec<ServerMessage>>  = vec![];

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
            
                  // let (throttle, num_orders) = (throttle_nanos.unwrap_or(1_000_000_000), total_objects.unwrap_or(1_000));
                  // let (mean, sd, best_price_lvls) = (mean_price.unwrap_or(300.0), sd_price.unwrap_or(50.0), best_price_levels.unwrap_or(false));

                  tokio::spawn(process_start_message(tx.clone(), total_objects, mean_price, sd_price, best_price_levels, throttle_nanos));
               },
               ClientMessage::Stop => {
                println!(">>> {} requested STOP", who);
                // if let Some(handle) = process_handle.take() {
                //   handle.abort();
                // }
                // if tx.send(Message::Close(Some(CloseFrame {
                //   code: NORMAL,
                //   reason: Utf8Bytes::from_static("Client Requested STOP")
                // }))).await.is_err() { 
                //   println!("Could not send Close frame to channel after client STOP request!");
                // }

                break;
               }
              }
            },
            Message::Close(c) => {
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
              //println!("sending rem updates: {:?}", &batch.len());
              let update_msg = Message::text(serde_json::to_string(&batch).expect("serializing final server updates failed!"));
              if sender.send(update_msg).await.is_err() {
                break;
              }
            }
            rx.close();
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

            if batch.len() >= 1_000 {
              //println!("batch: {:?}", &batch.len());
              let update_msg = Message::text(serde_json::to_string(&batch).expect("serializing server updates failed!"));
              if sender.send(update_msg).await.is_err() {
                break;
              }
              batch.clear();
            }
          }


          /*Working Ver with time based batching
          Simulation::Data(mut data_updates) => {
            // << Send batched updates to the Websocket Client >>
            
            // extract the engine stats (1st element) and store
            if let ServerMessage::ExecutionStats(current_stat) = data_updates.remove(0) {
              ws_batcher.engine_stats.extend(current_stat);
            }

            // extract new trades (if any) which would be the last elem
            if let Some(ServerMessage::Trades(t)) = data_updates.last() {
              //println!("new trades: {:?}", &t);
              if let Some(ServerMessage::Trades(new_trades)) = data_updates.pop() {
                ws_batcher.trades.extend(new_trades);
              }
            };

            // check if its time to send update
            if ws_batcher.should_send() {
              
              data_updates.push(ServerMessage::ExecutionStats(ws_batcher.engine_stats.clone()));
              
              if !ws_batcher.trades.is_empty() {
                data_updates.push(ServerMessage::Trades(ws_batcher.trades.clone()))
              };
              
              let update_msg = Message::text(serde_json::to_string(&data_updates).expect("serializing server updates failed!"));
              if sender.send(update_msg).await.is_err() {
                break;
              }
              // reset the timer & clear the buffer
              ws_batcher.engine_stats.clear();
              ws_batcher.trades.clear();
              ws_batcher.reset_timer();

            }
          },
          */
        }
        
        /*Working Ver with serialized msg on channel
        if let Message::Close(c) = msg {
          if let Some(cf) = c {
            println!("CloseChannel frame: code {} and reason: {}", cf.code, cf.reason)
          } else{
            println!(">>> Somehow got close channel request without CloseFrame");
          }
          rx.close();
          break;
        } else {
          // Sending to the Client>>
          if last_update.elapsed() >= ws_send_freq {

            if sender.send(msg).await.is_err() {
            break;
          }
          last_update = Instant::now();
        }

          /* Working ver with batches
          if sender.feed(msg).await.is_err(){
            println!("sending simulator updates via feed to dioxus server failed!");
            break;
          }

          batch_cntr += 1;
          
          if batch_cntr >= 100 {
            sender.flush().await.expect("flushing failed while sending batched updates to dioxus server!");
            batch_cntr = 0;
          }
          */
        }
        */

      }
    }
  }

  println!("Websocket context {} destroyed", who);
}

async fn process_start_message(tx: Sender<Simulation>, num_orders: usize, mean_price: f64, sd_price: f64, best_price_lvls: bool, throttle: u64) {
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
    // interval.tick().await;
    // generate and process the orders
    simulator.generate_orders();
    let updates = simulator.generate_updates(idx);

    if tx.send(Simulation::Data(updates)).await.is_err() {
      panic!("receiver half of channel dropped!");
    };
    
  }
  //println!("trades: {:?}", simulator.book.executed_orders);
  println!("[INFO] Completed simulation (total trades: {:?}), sending close frame to channel", simulator.book.executed_orders.len());
  // if tx.send(Message::Close(Some(CloseFrame {
  //   code: NORMAL,
  //   reason: Utf8Bytes::from_static("Streaming complete")
  // }))).await.is_err() { 
  //   println!("Could not send Close frame to channel after simulation complete!");
  // }

  // sleep a bit before dropping the connection
  // sleep(Duration::from_millis(5)).await;

  if tx.send(Simulation::Complete).await.is_err() { 
    panic!("Could not send close signal to channel after simulation was complete!");
  }
}