use std::{collections::{BTreeMap, HashMap}, io::Read};
use dioxus::{logger::tracing::{info, warn, error}, prelude::*};
use flate2::read::DeflateDecoder;
use gloo_net::websocket::{futures::WebSocket, Message};
use futures_util::StreamExt;
use futures::{stream::SplitSink, SinkExt};
use tokio::sync::mpsc::Sender;


use crate::{
  pages::simulator::{DataUpdate, EngineStats, ExecutedOrders, View, HEALTH_CHECK_URL, WSS_URL},
  utils::{enginestats::get_cumlative_results, server::{HealthCheckResponse, AppError, WsRequest, WsResponse}}
};

// static WS: GlobalSignal<Option<SplitSink<WebSocket, Message>>> = Signal::global(||None);

// helper to decompress data
fn decompress_data(data: &[u8]) -> Result<String, AppError> {
  let mut decoder = DeflateDecoder::new(data);
  let mut decompressed = String::new();
  decoder.read_to_string(&mut decompressed).map_err(|e| AppError::DecompressionFailed(e.to_string()))?;
  Ok(decompressed)
}

pub async fn handle_websocket(start_payload: Message,
  mut ws_conn: Signal<Option<SplitSink<WebSocket, Message>>>,
  update_tx: Sender<DataUpdate>,
  mut sim_completed: Signal<bool>,
  mut feed_killed: Signal<bool>,
  mut view: Signal<View>,
  all_engine_stats: Signal<Vec<EngineStats>>,
  all_executed_orders: Signal<Vec<ExecutedOrders>>,
  mut cuml_latency: Signal<Vec<i64>>,
  mut cuml_latency_by_ordertype: Signal<HashMap<String, Vec<f64>>>,
  mut cuml_latency_by_avl_trade: Signal<BTreeMap<(i64, i64), f64>>,
  qvals: Signal<Vec<f64>>) -> Result<(), AppError> {

  match reqwest::get(HEALTH_CHECK_URL).await {
    Ok(r) => {
        let json_response = r.json::<HealthCheckResponse>().await.expect("failed to deserialize healthcheck response");
        info!("health check succeeded with status: {}", json_response.code);
    },
    Err(e) => {
        return Err(AppError::ServerUnhealthy(e.to_string()))
    }
  };
  
  let ws = WebSocket::open(WSS_URL).map_err(|e| AppError::WsConnectionError(e.to_string()))?;

  let(mut write, mut read) = ws.split();
  match write.send(start_payload).await {
    Ok(_) => { 
        info!("START payload sent succ to server");
        // feed_killed.set(false);
        // view.set(View::Execution);
    },
    Err(e) => return Err(AppError::ConnectionFailed(e.to_string()))
    // error!("error {:?} occ sending START msg to server", e)
  }; 

  // store the conn in global signal
  *ws_conn.write() = Some(write);
  
  // Flag to track if we've received the first valid message
  let mut first_valid_message_received = false;

  // Receiving from backend axum server
  while let Some(Ok(server_msg)) = read.next().await {
    match server_msg {
      Message::Text(s) => {
        // info!("server text msg size: {:?}", std::mem::size_of_val(&*s));
        let batch = serde_json::from_str::<Vec<Vec<WsResponse>>>(&s).expect("error deserializing orderbook updates from server!");
        process_updates(batch, &mut first_valid_message_received, &mut ws_conn, &update_tx, &mut sim_completed, &mut feed_killed, &mut view, &all_engine_stats, &all_executed_orders, &mut cuml_latency, &mut cuml_latency_by_ordertype, &mut cuml_latency_by_avl_trade, &qvals).await?;
      },
      Message::Bytes(compressed_data) => {
        // info!("server byte msg size: {:?}", std::mem::size_of_val(&*compressed_data));
        let decompressed = decompress_data(&compressed_data)?;
        let batch = serde_json::from_str::<Vec<Vec<WsResponse>>>(&decompressed).expect("error deserializing decompressed orderbook updates from server!");
        process_updates(batch, &mut first_valid_message_received, &mut ws_conn, &update_tx, &mut sim_completed, &mut feed_killed, &mut view, &all_engine_stats, &all_executed_orders, &mut cuml_latency, &mut cuml_latency_by_ordertype, &mut cuml_latency_by_avl_trade, &qvals).await?;
      }
    }
  }
  feed_killed.set(true);
  Ok(())
}

async fn process_updates(batch: Vec<Vec<WsResponse>>,
  first_valid_message_received: &mut bool,
  ws_conn: &mut Signal<Option<SplitSink<WebSocket, Message>>>,
  update_tx: &Sender<DataUpdate>,
  sim_completed: &mut Signal<bool>,
  feed_killed: &mut Signal<bool>,
  view: &mut Signal<View>,
  all_engine_stats: &Signal<Vec<EngineStats>>,
  all_executed_orders: &Signal<Vec<ExecutedOrders>>,
  cuml_latency: &mut Signal<Vec<i64>>,
  cuml_latency_by_ordertype: &mut Signal<HashMap<String, Vec<f64>>>,
  cuml_latency_by_avl_trade: &mut Signal<BTreeMap<(i64, i64), f64>>,
  qvals: &Signal<Vec<f64>>) -> Result<(), AppError> {
    for updates in batch {
      for update in updates {

        if let WsResponse::RateLimitExceeded = update {
          return Err(AppError::RateLimitExceeded("order limit exceeded".to_string()))
        }
        
        if !*first_valid_message_received {
          feed_killed.set(false);
          view.set(View::Execution);
          *first_valid_message_received = true;
        }

        match update {
          WsResponse::PriceLevels { snapshot, bids, asks } => {
            update_tx.send(DataUpdate::PriceLevels { snapshot, bids, asks }).await.map_err(|e| AppError::WsChannelError(e.to_string()))?;
          },
          WsResponse::ExecutionStats(stats) => {
            update_tx.send(DataUpdate::PlotData(stats)).await.map_err(|e| AppError::WsChannelError(e.to_string()))?;
          },
          WsResponse::Trades(trades) => {
            update_tx.send(DataUpdate::Transactions(trades)).await.map_err(|e| AppError::WsChannelError(e.to_string()))?;
          },
          WsResponse::BestLevels { best_buy, best_sell } => {
            update_tx.send(DataUpdate::BestPrices { best_buy, best_sell }).await.map_err(|e| AppError::WsChannelError(e.to_string()))?;
          },
          WsResponse::Completed => {
            info!("setting sim completed to true!");
            let ack_msg = serde_json::to_string(&WsRequest::Ack).expect("error serializing acknowledgement message!");
            if let Some(mut w) = ws_conn.write().take() {
              w.send(Message::Text(ack_msg)).await.map_err(|e| AppError::WsConnectionError(e.to_string()))?;
            };

            sim_completed.set(true);
            let (lat, lat_by_ordertype, lat_by_avl_trades) = get_cumlative_results(all_engine_stats(), all_executed_orders(), qvals());
            cuml_latency.set(lat);
            cuml_latency_by_ordertype.set(lat_by_ordertype);
            cuml_latency_by_avl_trade.set(lat_by_avl_trades);

          },
          WsResponse::RateLimitExceeded => {
            // this is handled but keeping as a fallback
            view.set(View::Selector);
            feed_killed.set(true);
            return Err(AppError::RateLimitExceeded("order limit exceeded".to_string()));
          }
        }
      }
    }
    Ok(())
  }
