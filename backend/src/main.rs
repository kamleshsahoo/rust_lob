mod order_generator;
mod engine;

use std::{net::SocketAddr, ops::ControlFlow, time::Duration};
use axum::{extract::{ws::{close_code::NORMAL, CloseFrame, Message, Utf8Bytes, WebSocket}, ConnectInfo, WebSocketUpgrade}, response::IntoResponse, routing::any, Router};
use futures_util::{SinkExt, StreamExt};
use order_generator::gen::Simulator;
use serde::Deserialize;
use tokio::{net::TcpListener, sync::mpsc::{self, Sender}, time};


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
  
  let send_task = tokio::spawn(async move {
    while let Some(msg) = rx.recv().await {
      // sending to the Client
      if sender.send(msg).await.is_err() {
        break;
      }
    }

    println!("Closing connection to {:?}", who);
    if let Err(e) = sender.send(Message::Close(Some(CloseFrame {
      code: NORMAL,
      reason: Utf8Bytes::from_static("Streaming complete")
    }))).await {
      println!("Could not send Close due to {:?}", e);
    }

  });

  let recv_task = tokio::spawn(async move {
    while let Some(Ok(client_msg)) = receiver.next().await {
      if process_message(client_msg, who, tx.clone()).await.is_break() {
        break;
      }
    }
    // println!("recv task over aka all order updates produced. we dont abort send task yet");
  });

  /*TODO: see if need select!
  tokio::select! {
    _send = &mut send_task => {
      println!("send task over aka all orders sent to fronted!!");
      recv_task.abort()},
    _rcv = &mut recv_task => {
      println!("recv task over aka all order updates produced. we dont abort send task yet");
      // send_task.abort()},
    }
  }
  */

  let (_r, _s) = tokio::join!(recv_task, send_task);

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
          let snapshot = simulator.get_snapshot();
          let snapshot_msg = Message::text(serde_json::to_string(&snapshot).expect("serializing snapshot failed!"));
          if tx.send(snapshot_msg).await.is_err() {
            println!("receiver half of channel dropped when sending initial snapshot!");
            return ControlFlow::Break(());
          }

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
          // println!("[INFO] Simulation complete and now waiting for 5 secs...");
          // tokio::time::sleep(Duration::from_secs(5)).await;
          return ControlFlow::Break(());
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
