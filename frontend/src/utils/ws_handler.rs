use std::{collections::{BTreeMap, HashMap}, io::Read};
use dioxus::{logger::tracing::info, prelude::*};
use flate2::read::DeflateDecoder;
use gloo_net::websocket::{futures::WebSocket, Message, WebSocketError};
use futures_util::StreamExt;
use futures::{stream::SplitSink, SinkExt};
use tokio::sync::mpsc::Sender;

use crate::{
  pages::simulator::{DataUpdate, EngineStats, ExecutedOrders, View, HEALTH_CHECK_URL, WSS_URL},
  utils::{enginestats::get_cumlative_results, server::{HealthCheckResponse, AppError, WsRequest, WsResponse}}
};
use super::auth::AuthSignature;

// helper to decompress data
fn decompress_data(data: &[u8]) -> Result<String, AppError> {
  let mut decoder = DeflateDecoder::new(data);
  let mut decompressed = String::new();
  decoder.read_to_string(&mut decompressed).map_err(|e| AppError::DecompressionError(e.to_string()))?;
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
      let _json_response = r.json::<HealthCheckResponse>().await.expect("failed to deserialize healthcheck response");
      //info!("health check for ws succeeded with: {:?}", json_response);
    },
    Err(e) => return Err(AppError::ServerUnhealthy(e.to_string()))
  };

  let auth_signer = AuthSignature::new().await?;
  let timestamp = (js_sys::Date::now() / 1000.0) as u64;
  let signature = auth_signer.sign_with_key("/wslob", timestamp).await?;

  let ws_sec_protocols = [&timestamp.to_string(), &signature];
  let ws = WebSocket::open_with_protocols(WSS_URL, &ws_sec_protocols).map_err(|e| AppError::WsConnectionError(e.to_string()))?;

  let(mut write, mut read) = ws.split();
  
  match write.send(start_payload).await {
    Ok(_) => { 
      info!("Ws START payload sent to server");
    },
    Err(e) => return Err(AppError::WsConnectionError(e.to_string()))
  };

  // Flag to track to track first valid msg received
  let mut first_valid_message_received = false;
  // store the write part of connection in signal
  *ws_conn.write() = Some(write);

  // NOTE: we are not handling None case because the .next() returns None when
  // no more msgs are left in the stream, so probably fine to ignore it
  while let Some(msg_response) = read.next().await {
    match msg_response {
      Ok(server_msg) => {
        let batch: Vec<Vec<WsResponse>> = match server_msg {
          Message::Text(string_data) => serde_json::from_str::<Vec<Vec<WsResponse>>>(&string_data).map_err(|e| AppError::DeserializeError(e.to_string()))?,
          Message::Bytes(compressed_byte_data) => {
            let decompressed = decompress_data(&compressed_byte_data)?;
            serde_json::from_str::<Vec<Vec<WsResponse>>>(&decompressed).map_err(|e| AppError::DeserializeError(e.to_string()))?
          }
        };
        // process the batch
        process_updates(batch, &mut first_valid_message_received, &mut ws_conn, &update_tx, &mut sim_completed, &mut feed_killed, &mut view, &all_engine_stats, &all_executed_orders, &mut cuml_latency, &mut cuml_latency_by_ordertype, &mut cuml_latency_by_avl_trade, &qvals).await?
      },
      Err(e) => {
        match e {
          WebSocketError::ConnectionClose(close_evt) => {
            info!("Ws connection closed by server : {:?}", close_evt)
          },
          _ => {
            //error!("Ws error {:?}", e);
            return Err(AppError::AuthorizationError(format!("Ws connection rejected with {:?}", e)));
          }
        }
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
            //info!("setting sim completed to true!");
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
