use dioxus::{logger::tracing::{error, info}, prelude::*};
use dioxus::document;
use std::collections::{BTreeMap, HashMap};
use rust_decimal::Decimal;
use futures::{stream::SplitSink, SinkExt};
use futures_util::StreamExt;
use gloo_net::websocket::{futures::WebSocket, Message};
use serde::Deserialize;
use tokio::sync::mpsc;
use web_sys::{window, Performance};

use crate::{
    components::{modeselect::ModeSelector, results::{CumlExecutionView, ExecutionView}, toast::ErrorToast},
    utils::{
        enginestats::{get_latency_by_avl_trades, get_latency_by_ordertype}, priceupdate::PriceLevelProcessor, server::{AppError, WsRequest},
        ws_handler::handle_websocket
    }
};
enum Action {
  Start,
  Stop
}

#[derive(Debug, Deserialize, Clone)]
pub struct EngineStats {
  pub order_type: String,
  pub latency: i64,
  pub avl_rebalances: i64,
  pub executed_orders_cnt: usize
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ExecutedOrders {
  pub price: Decimal,
  pub volume: u64,
  pub aggresive_order_id: u64,
  pub passive_order_id: u64,
}

pub const ORDERBOOK_LEVELS: usize = 20;

#[derive(Clone)]
pub struct PlotPropsState {
  pub latency_cutoff: Signal<i64>,
  pub frequency_cutoff: Signal<i64>,
  pub avg_latency_cutoff: Signal<i64>
}

#[derive(Debug)]
pub enum DataUpdate {
  PlotData(EngineStats),
  PriceLevels { snapshot: bool, bids: Vec<(Decimal, u64)>, asks: Vec<(Decimal, u64)> },
  Transactions(Vec<ExecutedOrders>),
  BestPrices {best_buy: Option<Decimal>, best_sell: Option<Decimal>}
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
  Simulation,
  Upload
}

#[derive(Debug, Clone, PartialEq)]
pub enum View {
  Selector,
  Execution
}

struct UpdateTimings {
  price_levels: f64,
  latency3d: f64,
  latency: f64,
  trades: f64,
  spread: f64,
}

enum DomElement {
  PriceLevels,
  Latency3d,
  Latency,
  Trades,
  Spread
}
struct DataCollector {
  perf_inst: Performance,
  last_updates: UpdateTimings,
  timeout_price_levels: f64,
  timeout_latency3d: f64,
  timeout_latency: f64,
  timeout_trades: f64,
  timeout_spread: f64,
}

impl DataCollector {
  fn new(timeout_price_levels: f64, timeout_latency3d: f64, timeout_latency: f64, timeout_trades: f64, timeout_spread: f64,) -> Self {
      let window = window().expect("window should exist in this context");
      let perf_inst = window.performance().expect("performance should be available");
      let init_update = (&perf_inst).now();

      Self { 
          perf_inst,
          last_updates: UpdateTimings { 
              price_levels: init_update,
              latency3d: init_update,
              latency: init_update,
              trades: init_update,
              spread: init_update,
          },
          timeout_price_levels,
          timeout_latency3d,
          timeout_latency,
          timeout_trades,
          timeout_spread,
      }
  }

  fn elapsed(&self, last_update: f64) -> f64 {
      self.perf_inst.now() - last_update       
  }
  
  fn should_update(&self, element: DomElement) -> bool {
      match element {
          DomElement::PriceLevels => self.elapsed(self.last_updates.price_levels) >= self.timeout_price_levels,
          DomElement::Latency3d => self.elapsed(self.last_updates.latency3d) >= self.timeout_latency3d,
          DomElement::Latency => self.elapsed(self.last_updates.latency) >= self.timeout_latency,
          DomElement::Trades => self.elapsed(self.last_updates.trades) >= self.timeout_trades,
          DomElement::Spread => self.elapsed(self.last_updates.spread) >= self.timeout_spread,
      }
  }

  fn reset_timer(&mut self, element: DomElement) {
      match element {
          DomElement::PriceLevels => self.last_updates.price_levels = self.perf_inst.now(),
          DomElement::Latency3d => self.last_updates.latency3d = self.perf_inst.now(),
          DomElement::Latency => self.last_updates.latency = self.perf_inst.now(),
          DomElement::Trades => self.last_updates.trades = self.perf_inst.now(),
          DomElement::Spread => self.last_updates.spread = self.perf_inst.now(),
      }
  }
}

pub const HEALTH_CHECK_URL: &str = env!("HEALTH_CHECK_URL");
pub const SMALL_UPLOAD_URL: &str = env!("SMALL_UPLOAD_URL");
pub const LARGE_UPLOAD_URL: &str = env!("LARGE_UPLOAD_URL");
pub const WSS_URL: &str = env!("WSS_URL"); 

#[component]
pub fn Simulator() -> Element {
    
    let mut ws_conn: Signal<Option<SplitSink<WebSocket, Message>>> = use_signal(||None);
    let mut engine_stats: Vec<EngineStats> = vec![];
    let mut engine_stats3d: Vec<EngineStats> = vec![];
    let mut all_engine_stats: Signal<Vec<EngineStats>> = use_signal(||vec![]);
    // quantile values for error bars 
    let qvals: Signal<Vec<f64>> = use_signal(||vec![0.15, 0.85]);

    let plot_props_state = use_context_provider(|| PlotPropsState {
        latency_cutoff: Signal::new(20_000),
        frequency_cutoff: Signal::new(4_000),
        avg_latency_cutoff: Signal::new(35_000),
    });
    
    let mut form_data: Signal<HashMap<String, FormValue>> = use_signal(||HashMap::new());
    let is_valid_sim_settings = use_signal(||true);
    // BID signals
    let mut bid_lvls: Signal<Vec<(Decimal, u64, u64, f32)>> = use_signal(||Vec::<(Decimal, u64, u64, f32)>::new());
    let mut raw_bids: Signal<Vec<(Decimal, u64)>> = use_signal(||Vec::<(Decimal, u64)>::new());
    // ASK signals
    let mut ask_lvls: Signal<Vec<(Decimal, u64, u64, f32)>> = use_signal(||Vec::<(Decimal, u64, u64, f32)>::new());
    let mut raw_asks: Signal<Vec<(Decimal, u64)>> = use_signal(||Vec::<(Decimal, u64)>::new());
    // Best Bid, Ask and Spread signals
    let (mut best_bid, mut best_ask, mut spread) = (use_signal(||None), use_signal(||None), use_signal(||None));
    // signals for latency plots
    let mut latency: Signal<Vec<i64>> = use_signal(||Vec::new());
    let mut latency_by_ordertype: Signal<HashMap<String, Vec<f64>>> = use_signal(||HashMap::new());
    let mut latency_by_avl_trade: Signal<BTreeMap<(i64, i64), f64>> = use_signal(||BTreeMap::new());
    let mut cuml_latency: Signal<Vec<i64>> = use_signal(||Vec::new());
    let mut cuml_latency_by_ordertype: Signal<HashMap<String, Vec<f64>>> = use_signal(||HashMap::new());
    let mut cuml_latency_by_avl_trade: Signal<BTreeMap<(i64, i64), f64>> = use_signal(||BTreeMap::new());
    // signals for executed orders
    let mut fulfilled_orders: Signal<Vec<ExecutedOrders>> = use_signal(||Vec::with_capacity(25));
    let mut executed_orders: Signal<Vec<ExecutedOrders>> = use_signal(||vec![]);
    let mut all_executed_orders: Signal<Vec<ExecutedOrders>> = use_signal(||vec![]);
    //Signals for showing Simulation or File upload settings 
    let mode: Signal<Mode> = use_signal(||Mode::Simulation); 
    let mut sim_completed: Signal<bool> = use_signal(||false);
    let mut show_cumlative_results: Signal<bool> = use_signal(||false);
    let mut feed_killed: Signal<bool> = use_signal(||true);
    let mut view: Signal<View> = use_signal(||View::Selector);
    //let mut server_error: Signal<Option<ServerError>> = use_signal(||None);

    // Channel to send messages from WebSocket to the UI
    let (update_tx, mut update_rx) = mpsc::channel::<DataUpdate>(1_000_000);
    
    // React to client request
    let ws_client = use_coroutine(move|mut rx| {
        let update_tx = update_tx.clone();

        async move {
            while let Some(action) = rx.next().await {
                match action {
                    Action::Start => {
                        /*create the payload from formdata */
                        let current_form_data = form_data();
                        //info!("raw formdata: {:?}", current_form_data);    
                        
                        let orders = current_form_data.get("orders").map_or(50_000, |v| v.as_value().parse::<usize>().unwrap_or(50_000));
                        let add_prob= current_form_data.get("add_prob").map_or(0.0, |v| v.as_value().parse::<f32>().unwrap_or(0.0));
                        let modify_prob= current_form_data.get("modify_prob").map_or(0.6, |v| v.as_value().parse::<f32>().unwrap_or(0.6));
                        let cancel_prob= current_form_data.get("cancel_prob").map_or(0.4, |v| v.as_value().parse::<f32>().unwrap_or(0.4));
                        // NOTE: the order should be ADD, CANCEL, MODIFY probs
                        let order_probs = vec![add_prob, cancel_prob, modify_prob];
                        
                        let mean_price = current_form_data.get("mean_price").map_or(250.0, |v| v.as_value().parse::<f64>().unwrap_or(250.0));
                        let sd_price = current_form_data.get("sd_price").map_or(20.0, |v| v.as_value().parse::<f64>().unwrap_or(20.0));
                        let price_lvls_display = current_form_data.get("price_lvl").map_or(false,|v| v.as_value().parse::<bool>().unwrap_or(false));

                        //Before starting the engine, clear states
                        bid_lvls.set(vec![]);
                        ask_lvls.set(vec![]);
                        raw_bids.set(vec![]);
                        raw_asks.set(vec![]);
                        fulfilled_orders.set(Vec::with_capacity(25));
                        executed_orders.set(vec![]);
                        all_executed_orders.clear();
                        all_engine_stats.clear();
                        latency.set(vec![]);
                        latency_by_ordertype.set(HashMap::new());
                        latency_by_avl_trade.set(BTreeMap::new());
                        cuml_latency.set(vec![]);
                        cuml_latency_by_ordertype.set(HashMap::new());
                        cuml_latency_by_avl_trade.set(BTreeMap::new());
                        best_bid.set(None);
                        best_ask.set(None);
                        spread.set(None);
                        sim_completed.set(false);
                        show_cumlative_results.set(false);
                        form_data.write().clear();
                        //server_error.set(None);

                        // TODO: better conditioning for x-y limits of latency histogram
                        if sd_price < 10.0 {
                            use_context::<PlotPropsState>().latency_cutoff.set(10_000);
                            use_context::<PlotPropsState>().frequency_cutoff.set(6_000);
                            use_context::<PlotPropsState>().avg_latency_cutoff.set(18_000);
                        } else if sd_price < 30.0 {
                            use_context::<PlotPropsState>().latency_cutoff.set(12_000);
                            use_context::<PlotPropsState>().frequency_cutoff.set(5_000);
                            use_context::<PlotPropsState>().avg_latency_cutoff.set(25_000);
                        } else {
                            use_context::<PlotPropsState>().latency_cutoff.set(20_000);
                            use_context::<PlotPropsState>().frequency_cutoff.set(4_000);
                            use_context::<PlotPropsState>().avg_latency_cutoff.set(35_000);
                        }
                        
                        let client_msg = WsRequest::Start { total_objects: orders, mean_price, sd_price, order_probs, best_price_levels: price_lvls_display};
                        //info!("prepped formdata: {:?}", &client_msg);

                        let start_payload =  Message::Text(serde_json::to_string(&client_msg).expect("error deserializing START message!"));
                        
                        spawn({
                            let update_tx = update_tx.clone();
                            async move {
                                if let Err(ws_err) = handle_websocket(start_payload, ws_conn, update_tx, sim_completed, feed_killed, view, all_engine_stats, all_executed_orders, cuml_latency, cuml_latency_by_ordertype, cuml_latency_by_avl_trade, qvals).await {
                                    //error!("Websocket handler error: {:?}", ws_err);
                                    
                                    match ws_err {
                                        AppError::RateLimitExceeded(_) => 
                                        document::eval(r#"
                                        var x = document.getElementById("server-rl-toast");
                                        x.classList.add("show");
                                        setTimeout(function(){{x.classList.remove("show");}}, 2000);
                                        "#),
                                        _ => document::eval(r#"
                                        var x = document.getElementById("server-down-toast");
                                        x.classList.add("show");
                                        setTimeout(function(){{x.classList.remove("show");}}, 2000);
                                        "#)
                                    };
                                };
                            }
                        });
                    },
                    Action::Stop => {
                        //warn!("User Clicked Stop");
                        if let Some(mut ws) = ws_conn.write().take() {
                            let stop_msg = serde_json::to_string(&WsRequest::Stop).expect("error serializing STOP message!"); 
                            match ws.send(Message::Text(stop_msg)).await {
                                Ok(_) => {
                                    info!("STOP msg sent succ to server");
                                    //*FEEDKILLED.write() = true;                         
                                    feed_killed.set(true);                         
                                },
                                Err(e) => error!("error {:?} occured sending Ws STOP msg to server", e)
                            };
                        }
                    }
                }
            }
        }
    });

    /* Update the ui states */
    spawn(async move {
        let mut pricelvl_proc = PriceLevelProcessor::new();
        
        let mut data_collector = DataCollector::new(100.0, 300.0, 300.0, 50.0, 45.0);

        while let Some(update) = update_rx.recv().await {
            match update {
                DataUpdate::PlotData(plot_data) => {

                    all_engine_stats.push(plot_data.clone());
                    engine_stats.push(plot_data.clone());
                    engine_stats3d.push(plot_data);

                    if data_collector.should_update(DomElement::Latency) {
                        //info!("engine stats len: {:?}", &engine_stats.len());
                        let lat = engine_stats.iter().map(|e| e.latency).collect::<Vec<i64>>();
                        latency.set(lat);

                        let lat_by_ordertype = get_latency_by_ordertype(&engine_stats, &qvals());
                        latency_by_ordertype.set(lat_by_ordertype);

                        engine_stats.clear();
                        data_collector.reset_timer(DomElement::Latency);
                    }

                    if data_collector.should_update(DomElement::Latency3d) {
                        let lat_by_avl_trades = get_latency_by_avl_trades(&engine_stats3d);
                        //info!("lat by avl & trades: {:?}", &lat_by_avl_trades);
                        latency_by_avl_trade.set(lat_by_avl_trades);

                        engine_stats3d.clear();
                        data_collector.reset_timer(DomElement::Latency3d);
                    }
                },
                DataUpdate::PriceLevels{snapshot, bids, asks} => {
                    if snapshot {
                        pricelvl_proc.updater(snapshot, bids, asks, raw_bids, raw_asks, bid_lvls, ask_lvls)
                    } else {
                        if data_collector.should_update(DomElement::PriceLevels) {
                            pricelvl_proc.updater(snapshot, bids, asks, raw_bids, raw_asks, bid_lvls, ask_lvls);
                            data_collector.reset_timer(DomElement::PriceLevels);
                        }
                    }
                },
                DataUpdate::Transactions(trades) => {
                    all_executed_orders.extend(trades.clone());
                    executed_orders.write().extend(trades);
                    let curr_len = executed_orders.len();

                    if curr_len > 0 && data_collector.should_update(DomElement::Trades) {
                        //info!("executed order updatetr: {:?}", executed_orders());
                        // only keep last 25 trades
                        if curr_len > 25 {
                            let to_remove = curr_len - 25;
                            executed_orders.write().drain(0..to_remove);
                        } 
                        fulfilled_orders.set(executed_orders());
                        data_collector.reset_timer(DomElement::Trades);
                    }
                },
                DataUpdate::BestPrices { best_buy, best_sell } => {
                    if data_collector.should_update(DomElement::Spread) {
                        if let (Some(bb), Some(bs)) = (best_buy, best_sell) {
                            let current_spread = (bb-bs).abs();
                            spread.set(Some(current_spread));
                        }
                        best_bid.set(best_buy);
                        best_ask.set(best_sell);
                        data_collector.reset_timer(DomElement::Spread);
                    }
                }
            }
        }
    });

    static CSS: Asset = asset!("/assets/sim.css");

    rsx! {
        document::Link { rel: "stylesheet", href: CSS },
        div {
            class: "sim-app-container",
            onmounted: move |_evt| {
                document::eval(r#"
                function loadScript(src, callback) {
                const scriptElem = document.createElement('script');
                scriptElem.src = src;
                scriptElem.async = true;
                scriptElem.onload = callback;
                scriptElem.onerror = function() {
                  console.error(`Error loading script: ${src}`);
                };
                document.head.appendChild(scriptElem);
                }

                loadScript('https://cdn.jsdelivr.net/npm/echarts@5.6.0/dist/echarts.min.js', function() {
                    //console.log('echarts script has been loaded and executed');
                    loadScript('https://cdn.jsdelivr.net/npm/echarts-gl@2.0.9/dist/echarts-gl.min.js', function() {
                    //console.log('echarts gl script has been loaded and executed');
                    });
                });
                "#);
            },
            if view() == View::Selector {
                ModeSelector {mode, form_data, is_valid_sim_settings},
                if mode() == Mode::Simulation {
                    //Start simulation button
                    div {
                        id: "sim-start-btn",
                        class: "center mt-6",
                        button {
                            class: "button button-start-sim",
                            onclick: move|_evt| {
                                // info!("sending start signal to WebSocket server")
                                ws_client.send(Action::Start)
                            },
                            "Start Execution"
                        }
                    }
                }
            } else if view() == View::Execution {
                div {
                    class: "controls",
                    /* NOTE: may require Restart button */
                    // button {
                    //     class: if feed_killed() {"button"} else {"button button-danger"},
                    //     onclick: move |_evt| if feed_killed() { ws_client.send(Action::Start) } else { ws_client.send(Action::Stop) },
                    //     if feed_killed() {"Restart" } else { "Stop" }
                    // },
                    div {
                        class: "control-group",
                        if feed_killed() {
                            button {
                                class: "button button-mode",
                                onclick: move |_evt| view.set(View::Selector) ,
                                "Change settings/mode"
                            }
                        } else {
                            button { 
                                class: "button button-danger",
                                onclick: move|_evt| ws_client.send(Action::Stop),
                                "Stop"
                            }
                        }
                        if sim_completed() && !show_cumlative_results() {
                            button {
                                class: "button button-show-final",
                                onclick: move|_evt| show_cumlative_results.set(true),
                                "Show final results"
                            }
                        }
                    }
                }
                if !show_cumlative_results() {
                    ExecutionView {
                        bid_lvls,
                        ask_lvls,
                        latency,
                        fulfilled_orders,
                        latency_by_ordertype,
                        latency_by_avl_trade,
                        best_bid,
                        best_ask,
                        spread
                    }
                } else {
                    CumlExecutionView { 
                        cuml_latency: cuml_latency(),
                        cuml_latency_by_ordertype: cuml_latency_by_ordertype(),
                        cuml_latency_by_avl_trade: cuml_latency_by_avl_trade()
                    }
                }
            },
            ErrorToast { id: "server-down-toast", content: "SERVER IS DOWN! Try again later." },
            ErrorToast { id: "server-rl-toast", content: "Max order limit reached! Please try again in some time."}
        }
    }
}