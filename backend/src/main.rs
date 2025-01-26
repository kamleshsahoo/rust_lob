mod order_generator;
mod engine;

use std::{net::SocketAddr, ops::ControlFlow, time::Duration};
use axum::{extract::{ws::{Message, WebSocket}, ConnectInfo, WebSocketUpgrade}, response::IntoResponse, routing::any, Router};
use futures_util::{SinkExt, StreamExt};
use order_generator::gen::Simulator;
use serde::Deserialize;
use tokio::{net::TcpListener, sync::mpsc::{self, Sender}, time};


/*
fn weighted_idx_draws(n: usize) -> u128 {
    let start = Instant::now();

    let weights = [2, 399999, 599999]; // ADD, CANCEL, MODIFY
    //let choices = ["ADD", "CANCEL", "MODIFY"];
    let dist = WeightedIndex::new(&weights).unwrap();
    // let weights = vec![0, 4, 6];
    // let dist = WeightedAliasIndex::new(weights).unwrap();
    let mut rng = thread_rng();

    let mut occ = HashMap::new();
    
    for _ in 0..n {
        match dist.sample(&mut rng) {
            0 => *occ.entry("ADD").or_insert(0) += 1,
            1 =>  *occ.entry("CANCEL").or_insert(0) += 1,
            2 => *occ.entry("MODIFY").or_insert(0) += 1,
            _ => panic!("not possible case!!")
        }
    }
    let duration = start.elapsed().as_millis();

    println!("** occurences map: {:?}", occ);
    duration
}
*/

/*USE tHIS ONE */
/*
fn normal_draws(n: usize) -> u128{
    let start = Instant::now();
    let action_distr = Uniform::new(0.0, 1.0);
    let action_probs = vec![0.0, 0.4, 0.6];  // ADD, CANCEL, MODIFY
    let cuml_probs: Vec<f32> = action_probs.into_iter().scan(0.0, |acc, x| {
        *acc += x;
        Some(*acc)
      }).collect();

    // let cuml_probs: Vec<f32> = vec![0.0, 0.4 , 1.0];
    //println!("cumulative probs: {:?}", cuml_probs);
    
    let mut rng = thread_rng();

    let mut occ = HashMap::new();

    for _ in 0..n {
      
      let rand_num = action_distr.sample(&mut rng);
    //   let action_idx = cuml_probs.binary_search_by(|entry| match entry.partial_cmp(&rand_num) {
    //     Some(Ordering::Equal) => Ordering::Greater,
    //     Some(ord) => ord,
    //     None => panic!("comparison failed!!")
    //   }).unwrap_err(); // since we never return Equality we always get Err(idx)
    let action_idx = cuml_probs.iter().position(|cumsum| rand_num <= *cumsum).unwrap();

      match action_idx {
        0 => *occ.entry("ADD").or_insert(0) += 1,
        1 => *occ.entry("CANCEL").or_insert(0) += 1,
        2 => *occ.entry("MODIFY").or_insert(0) += 1,
        _ => panic!("error choosing a order type!!")
      }
    
    }
    let duration = start.elapsed().as_millis();

    println!("** occurences map: {:?}", occ);
    duration
}
*/
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
  Start { 
      // client_name: String, 
      total_objects: Option<usize>,  // Optional, defaults to 10
      throttle_nanos: Option<u64>, // Optional, defaults to 1000ns
      mean_price: Option<f64>,  // Optional, defaults to 300.0
      sd_price: Option<f64>,  // Optional, defaults to 50.0
      best_price_levels: Option<bool> // whether to show best bids and asks, defaults to false
  },
  Stop,
}


#[tokio::main]
async fn main() {
    
  /*TESTS
  let (mut normal_t, mut weighted_t) = (Vec::new(), Vec::new());
  let n = 5_000_000;
  println!("==== Starting (10 x 5_000_000) normal draws ====");
  for _ in 0..10 {
      normal_t.push(normal_draws(n));
  }
  println!("==== Starting (10 x 5_000_000) weighted index draws ====");
  for _ in 0..10 {
      weighted_t.push(weighted_idx_draws(n));
  }
  let normal_avg: f32 = normal_t.iter().sum::<u128>() as f32/10.0;
  let weighted_avg: f32 = weighted_t.iter().sum::<u128>() as f32/10.0;
  println!("Normal draws mean time: {:.2?}", normal_avg);
  println!("Weighted draws mean time: {:.2?}", weighted_avg);
  */

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
  //TODO: check if unbounded channel can be used
  let (tx, mut rx) = mpsc::channel(1_000_000);

  let mut send_task = tokio::spawn(async move {
    while let Some(msg) = rx.recv().await {
      // sending to the Client
      if sender.send(msg).await.is_err() {
        break;
      }
    }
  });

  let mut recv_task = tokio::spawn(async move {
    //let mut cnt = 0;
    while let Some(Ok(client_msg)) = receiver.next().await {
      if process_message(client_msg, who, tx.clone()).await.is_break() {
        break;
      }
    }
    //cnt
  });

  tokio::select! {
    _send = &mut send_task => recv_task.abort(),
    _rcv = &mut recv_task => send_task.abort(),
  }

  println!("Websocket context {} destroyed", who);
}

async fn process_message(msg: Message, who: SocketAddr, tx: Sender<Message> ) -> ControlFlow<(), ()> {
  match msg {
    Message::Text(t) => {
      println!(">>> {} sent string: {:?}", who, t);
      let client_msg = serde_json::from_str::<ClientMessage>(t.as_str());
      match client_msg {
        Ok(ClientMessage::Start { total_objects, throttle_nanos, mean_price, sd_price , best_price_levels }) => {
          
          println!("client payload\ntotal objects: {:?} nanos: {:?} mean: {:?} sd: {:?} show best price levels: {:?}", total_objects, throttle_nanos, mean_price, sd_price, best_price_levels);

          let (throttle, num_orders) = (throttle_nanos.unwrap_or(1_000_000_000), total_objects.unwrap_or(10));
          let (mean, sd) = (mean_price.unwrap_or(300.0), sd_price.unwrap_or(50.0));

          let mut simulator = Simulator::new(mean, sd, best_price_levels.unwrap_or(false));
          //TODO: see if we need to add throttling
          let mut interval = time::interval(Duration::from_nanos(throttle));

          // seed the orderbook with 10k ADD limit orders
          simulator.seed_orderbook(10_000);

          println!("[INFO] Starting simulation");
          for idx in 0..num_orders {

            // generate and process the orders
            simulator.generate_orders();
            let updates = simulator.generate_updates(idx);
            let msg = Message::text(serde_json::to_string(&updates).expect("serializing server updates failed!"));
            if tx.send(msg).await.is_err() {
              println!("receiver half of channel dropped!");
              break;
            };
          }
          // println!("engine stats: {:?}\nengine stats offset: {:?}", simulator.engine_stats, simulator.engine_stats_offset);
        },
        Ok(ClientMessage::Stop) => {
          println!(">>> {} requested STOP", who);
          return ControlFlow::Break(())
        },
        Err(e) => {
          println!(">>> {} errored with: {:?}, when parsing user message to ClientMessage", who, e);
          return ControlFlow::Break(())
        }
      }
    },
    Message::Close(c) => {
      if let Some(cf) = c {
        println!(">>> {} sent close with code {} and reason {}", who, cf.code, cf.reason)
      } else {
        println!(">>> {} somehow sent close message without CloseFrame", who);
      }
      return ControlFlow::Break(());
    },
    Message::Binary(d) => {
      println!(">>> {} sent {} bytes: {:?}", who, d.len(), d);
    },
    Message::Pong(v) => {
      println!(">>> {} sent pong with {:?}", who, v);
    },
    Message::Ping(v) => {
      println!(">>> {} sent ping with {:?}", who, v);
    },
  }
  ControlFlow::Continue(())
}
