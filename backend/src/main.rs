mod order_generator;
mod engine;

use std::{io, net::SocketAddr, ops::ControlFlow, time::Duration};
// use async_stream::{stream, try_stream};
use axum::{extract::{ws::{close_code::NORMAL, CloseFrame, Message, Utf8Bytes, WebSocket}, ConnectInfo, WebSocketUpgrade}, response::IntoResponse, routing::any, Router};
// use futures::{pin_mut, stream::SplitSink};
// use futures_util::{SinkExt, Stream, StreamExt};
use futures_util::{SinkExt, StreamExt};
use order_generator::gen::Simulator;
use serde::Deserialize;
//use tokio::{net::TcpListener, sync::mpsc::{self, Receiver, Sender}, time};
use tokio::{net::TcpListener, sync::mpsc::{self, Sender}, time};


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

                  tokio::spawn(process_start_message_v2(tx.clone(), total_objects, mean_price, sd_price, best_price_levels, throttle_nanos));
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
        
        if let Message::Close(c) = msg {
          if let Some(cf) = c {
            println!("CloseChannel frame: code {} and reason {}", cf.code, cf.reason)
          } else{
            println!(">>> Somehow got close channel request without CloseFrame");
          }
          rx.close();
          break;
        } else {
          // Sending to the Client>>
          // TODO: see if we can use send_all to batch and send 
          if sender.send(msg).await.is_err() {
            break;
          }
        }
      }
    }
  }

 /*
  while let Some(Ok(client_msg)) = receiver.next().await {
    match client_msg {
      Message::Text(t) => {
        println!(">>> {} sent string: {:?}", who, t);
        let payload = serde_json::from_str::<ClientMessage>(t.as_str());

        match payload {
          Ok(ClientMessage::Start { total_objects, throttle_nanos, mean_price, sd_price , best_price_levels }) => {
            
            println!("client payload\ntotal objects: {:?} nanos: {:?} mean: {:?} sd: {:?} show best price levels: {:?}", total_objects, throttle_nanos, mean_price, sd_price, best_price_levels);
            
            let (throttle, num_orders) = (throttle_nanos.unwrap_or(1_000_000_000), total_objects.unwrap_or(1_000));
            let (mean, sd, best_price_lvls) = (mean_price.unwrap_or(300.0), sd_price.unwrap_or(50.0), best_price_levels.unwrap_or(false));

            tokio::spawn(process_start_message_v2(tx.clone(), num_orders, mean, sd, best_price_lvls, throttle));
            
          },
          Ok(ClientMessage::Stop) => {
            println!(">>> {} requested STOP", who);
            break;
          },
          Err(e) => {
            println!("client Payload error: {:?}", e);
            break;
          }
        }
      },
      Message::Close(c) => {break;},
      _ => { println!(">> {} sent Binary, Ping or Pong", who); }
    }



  }
  */


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

  println!("Websocket context {} destroyed", who);
}

/*TODO: send the msgs in stream 
async fn process_messages(
  mut sender: SplitSink<WebSocket, Message>,
  mut rx: Receiver<Message>
) -> Result<(), Box<dyn std::error::Error>> {

  let mut stream = receiver_stream(rx);
  pin_mut!(stream);
  
  let _s = sender.send_all(&mut stream).await?;

  Ok(())
}

fn receiver_stream(mut rx: Receiver<Message>) -> impl Stream<Item = Result<Message, Error>> {
  
  stream! {
    while let Some(msg) = rx.recv().await {
      match msg {
        Message::Close(_) => {
          break;
        }
        _ => yield Ok(msg),
      }
    }
  }
}
*/

async fn process_start_message_v2(tx: Sender<Message>, num_orders: usize, mean_price: f64, sd_price: f64, best_price_lvls: bool, throttle: u64) {
  //TODO: see if we need to add throttling
  // let mut interval = time::interval(Duration::from_nanos(throttle));
  
  let mut simulator = Simulator::new(mean_price, sd_price, best_price_lvls);
  // seed the orderbook with 10k ADD limit orders
  simulator.seed_orderbook(10_000);
  let snapshot = simulator.get_snapshot();
  let snapshot_msg = Message::text(serde_json::to_string(&snapshot).expect("serializing snapshot failed!"));
  if tx.send(snapshot_msg).await.is_err() {
    panic!("receiver half of channel dropped when sending initial snapshot!");
    // return ControlFlow::Break(());
  }

  println!("[INFO] Starting simulation");
  for idx in 0..num_orders {
    // interval.tick().await;
    // generate and process the orders
    simulator.generate_orders();
    let updates = simulator.generate_updates(idx);
    let msg = Message::text(serde_json::to_string(&updates).expect("serializing server updates failed!"));
    if tx.send(msg).await.is_err() {
      panic!("receiver half of channel dropped!");
      // break;
    };

  }
  println!("[INFO] Completed simulation, sending close frame to channel");
  if tx.send(Message::Close(Some(CloseFrame {
    code: NORMAL,
    reason: Utf8Bytes::from_static("Streaming complete")
  }))).await.is_err() { 
    println!("Could not send Close frame to channel after simulation complete!");
  }

}


/* DEPRECATED Simulator
async fn process_message_deprecated(msg: Message, who: SocketAddr, tx: Sender<Message> ) -> ControlFlow<(), ()> {
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
*/