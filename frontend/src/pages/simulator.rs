use dioxus::{logger::tracing::{error, info, warn}, prelude::*};
use std::{collections::{BTreeMap, HashMap}, sync::OnceLock};
use rust_decimal::Decimal;
use futures::{stream::SplitSink, SinkExt};
use futures_util::StreamExt;
use gloo_net::websocket::{futures::WebSocket, Message};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Sender};
use crate::utils::{enginestats::{get_latency_by_avl_trades, get_latency_by_ordertype}, priceupdate::PriceLevelProcessor};
use crate::components::{modeselect::ModeSelector, results::ExecutionView};
use web_sys::{window, Performance};

enum Action {
  Start,
  Stop
}

#[derive(Debug, Deserialize)]
// #[serde(tag = "type")]
pub enum ServerMessage {
PriceLevels { snapshot: bool, bids: Vec<(Decimal, u64)>, asks: Vec<(Decimal, u64)> },
Trades (Vec<ExecutedOrders>),
// EngineStats(Vec<EngineStats>)
ExecutionStats (EngineStats),
BestLevels {best_buy: Option<Decimal>, best_sell: Option<Decimal>}
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


fn conv_nano() -> &'static HashMap<&'static str, u64> {
  static HASHMAP: OnceLock<HashMap<&'static str, u64>> = OnceLock::new();
  HASHMAP.get_or_init(|| {
      let mut m = HashMap::new();
      m.insert("ms", 1_000_000);
      m.insert("μs", 1_000);
      m
  })
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ClientMessage {
Start { 
    // client_name: String, 
    total_objects: usize,  // Optional, defaults to 10
    throttle_nanos: u64, // Optional, defaults to 1000ns
    mean_price: f64,  // Optional, defaults to 300.0
    sd_price: f64,  // Optional, defaults to 50.0
    best_price_levels: bool // whether to show best bids and asks, defaults to false
},
Stop,
}

static WS: GlobalSignal<Option<SplitSink<WebSocket, Message>>> = Signal::global(||None);
static FEEDKILLED: GlobalSignal<bool> = Signal::global(||true);
static VIEW: GlobalSignal<View> = Signal::global(||View::Selector);

const MAIN_CSS: Asset = asset!("/assets/main.css");
// const GEAR_SVG: Asset = asset!("/assets/gear.svg");
// const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

//TODO: send this to engine for the intial snapshot
pub const ORDERBOOK_LEVELS: usize = 20;
// const SLIDING_WINDOW: usize = 1_000;

#[derive(Clone)]
pub struct PlotPropsState {
  // xlim_hist: Signal<u128>,
  // ylim_hist: Signal<usize>
  pub latency_cutoff: Signal<i64>,
  pub frequency_cutoff: Signal<i64>
}

#[derive(Debug)]
enum DataUpdate {
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
enum View {
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
  // currently using same timeout for all plots
  // values can be 16.0(60fps) or 32.0(30fps)
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

async fn handle_websocket(ws_url: &str, start_payload: Message, update_tx: Sender<DataUpdate>) -> Result<(), Box<dyn std::error::Error>> {
  let ws = WebSocket::open(ws_url).expect("error opening WS connection!"); 
  let(mut write, mut read) = ws.split();

  match write.send(start_payload).await {
      Ok(_) => {
          info!("START payload sent succ to server");
          // is_feed_killed.set(false);
          *FEEDKILLED.write() = false;
          *VIEW.write() = View::Execution;
      },
      Err(e) => error!("error {:?} occ sending START msg to server", e)
  }; 

  // store the conn in global signal
  *WS.write() = Some(write);
  // Receiving from backend axum server
  while let Some(Ok(server_msg)) = read.next().await {
      match server_msg {
          Message::Text(s) => {
              // let updates = serde_json::from_str::<Vec<ServerMessage>>(&s).expect("error deserializing orderbook updates from server!");
              let batch = serde_json::from_str::<Vec<Vec<ServerMessage>>>(&s).expect("error deserializing orderbook updates from server!");
              for updates in batch {
                  for update in updates {
                      match update {
                          ServerMessage::PriceLevels { snapshot, bids, asks } => {
                              if let Err(e) = update_tx.send(DataUpdate::PriceLevels { snapshot, bids, asks }).await {
                                  warn!("sending price levels to data update channel erred: {:?}", e);
                              }
                          },
                          // ServerMessage::ExecutionStats(EngineStats { order_type, latency, avl_rebalances, executed_orders_cnt }) => {
                          ServerMessage::ExecutionStats(stats) => {
                              if let Err(e) = update_tx.send(DataUpdate::PlotData(stats)).await {
                                  warn!("sending latency to data update channel erred: {:?}", e);
                              };
                          },
                          ServerMessage::Trades(trades) => {
                              //info!("Executed trade\n:{:?}", trades);
                              if let Err(e) = update_tx.send(DataUpdate::Transactions(trades)).await {
                                  warn!("sending executed orders to data update channel erred: {:?}", e);
                              }
                          },
                          ServerMessage::BestLevels { best_buy, best_sell } => {
                              if let Err(e) = update_tx.send(DataUpdate::BestPrices { best_buy, best_sell }).await {
                                  warn!("sending best bid and ask level to data update channel erred: {:?}", e);
                              }
                          }
                      }
                  }
              }
          },
          Message::Bytes(b) => {info!("recvd bytes from server {:?}", b)}
      }
  }
  *FEEDKILLED.write() = true;
  Ok(())
}

#[component]
pub fn Simulator() -> Element {
    
    let WSS_URL = "ws://127.0.0.1:7575/wslob"; 
    // let plot_props_state = use_context_provider(|| PlotPropsState {
    //     xlim_hist: Signal::new(20_000u128),
    //     ylim_hist: Signal::new(500usize)
    // });

    let plot_props_state = use_context_provider(|| PlotPropsState {
        latency_cutoff: Signal::new(20_000),
        frequency_cutoff: Signal::new(500),
    });
    
    let form_data: Signal<HashMap<String, FormValue>> = use_signal(||HashMap::new());
    // BID signals
    let mut bid_lvls: Signal<Vec<(Decimal, u64, u64, f32)>> = use_signal(||Vec::<(Decimal, u64, u64, f32)>::new());
    let mut raw_bids: Signal<Vec<(Decimal, u64)>> = use_signal(||Vec::<(Decimal, u64)>::new());
    // ASK signals
    let mut ask_lvls: Signal<Vec<(Decimal, u64, u64, f32)>> = use_signal(||Vec::<(Decimal, u64, u64, f32)>::new());
    let mut raw_asks: Signal<Vec<(Decimal, u64)>> = use_signal(||Vec::<(Decimal, u64)>::new());
    let (mut best_bid, mut best_ask, mut spread) = (use_signal(||None), use_signal(||None), use_signal(||None));


    let mut engine_stats: Vec<EngineStats> = vec![];
    let mut engine_stats3d: Vec<EngineStats> = vec![];
    // quantile values for error bars 
    let qvals: Vec<f64> = vec![0.15, 0.85];

    let mut latency: Signal<Vec<i64>> = use_signal(||Vec::new());
    let mut latency_by_ordertype: Signal<HashMap<String, Vec<f64>>> = use_signal(||HashMap::new());
    let mut latency_by_avl_trade: Signal<BTreeMap<(i64, i64), f64>> = use_signal(||BTreeMap::new());

    //executed orders
    let mut fulfilled_orders: Signal<Vec<ExecutedOrders>> = use_signal(||Vec::with_capacity(25));
    let mut executed_orders: Signal<Vec<ExecutedOrders>> = use_signal(||vec![]);
    //Signals for showing Simulation or File upload settings 
    let mode: Signal<Mode> = use_signal(||Mode::Simulation); 

    // let view: Signal<View> = use_signal(||View::Selector); 
    // let show_simulation_settings: Signal<bool> = use_signal(||false); 

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
                        let mut current_form_data = form_data();
                        info!("raw formdata: {:?}", current_form_data);    
                        
                        let orders = current_form_data.remove("orders").map_or(50_000, |v| v.as_value().parse::<usize>().unwrap_or(50_000));
                        let unit = current_form_data.remove("units").map_or(String::from("μs"), |v| v.as_value());
                        let conv_factor = conv_nano().get(unit.as_str()).expect("failed to get the conv factor using form time units!"); 
                        let throttle = current_form_data.remove("time").map_or(100, |v: FormValue| v.as_value().parse::<u64>().unwrap_or(100) * conv_factor);
                        let mean_price = current_form_data.remove("mean_price").map_or(300.0, |v| v.as_value().parse::<f64>().unwrap_or(300.0));
                        let sd_price = current_form_data.remove("sd_price").map_or(50.0, |v| v.as_value().parse::<f64>().unwrap_or(50.0));
                        let price_lvls_display = current_form_data.remove("price_lvl").map_or(false,|v| v.as_value().parse::<bool>().unwrap_or(false));

                        //Before starting the engine, clear state
                        bid_lvls.set(vec![]);
                        ask_lvls.set(vec![]);
                        raw_bids.set(vec![]);
                        raw_asks.set(vec![]);
                        //fulfilled_orders.set(vec![]);
                        fulfilled_orders.set(Vec::with_capacity(25));
                        executed_orders.set(vec![]);
                        latency.set(vec![]);
                        latency_by_ordertype.set(HashMap::new());
                        latency_by_avl_trade.set(BTreeMap::new());
                        best_bid.set(None);
                        best_ask.set(None);
                        spread.set(None);

                        // TODO: better conditioning for x-y limits of latency histogram
                        if sd_price < 25.0 {
                            use_context::<PlotPropsState>().latency_cutoff.set(15_000);
                            use_context::<PlotPropsState>().frequency_cutoff.set(1_000);
                        } else {
                            use_context::<PlotPropsState>().latency_cutoff.set(20_000);
                            use_context::<PlotPropsState>().frequency_cutoff.set(500);
                        }
                        
                        let client_msg = ClientMessage::Start { total_objects: orders, throttle_nanos: throttle, mean_price, sd_price, best_price_levels: price_lvls_display };
                        info!("prepped formdata: {:?}", &client_msg);

                        let start_payload =  Message::Text(serde_json::to_string(&client_msg).expect("error deserializing START message!"));
                        
                        spawn({
                            let update_tx = update_tx.clone();
                            async move {
                                if let Err(ws_err) = handle_websocket(&WSS_URL, start_payload, update_tx).await {
                                    error!("Websocket handler error: {:?}", ws_err);
                                };
                            }
                        });
                        
                        // ** Moving out of spawn ** 
                        // spawn({
                        // let update_tx = update_tx.clone();
                        // async move {
                        // if let Err(ws_err) = handle_websocket(&WSS_URL, start_payload, update_tx.clone()).await {
                        //     error!("Websocket handler error: {:?}", ws_err);
                        // };
                        // }
                        // });

                    },
                    Action::Stop => {
                        warn!("User Clicked Stop");
                        if let Some(mut ws) = WS.write().take() {
                            let stop_msg = serde_json::to_string(&ClientMessage::Stop).expect("error deserializing STOP message!"); 
                            match ws.send(Message::Text(stop_msg)).await {
                                Ok(_) => {
                                    info!("STOP msg sent succ to server");
                                    // is_feed_killed.set(true);
                                    *FEEDKILLED.write() = true;                         
                                },
                                Err(e) => error!("error {:?} occured sending STOP msg to server", e)
                            };
                        }
                    }
                }
            }
        }
    });

    /*TOKIO SELECT with TIMING
    spawn(async move {
        let mut latency_proc = LatencyProcessor::new();
        let pricelvl_proc = PriceLevelProcessor::new(ORDERBOOK_LEVELS);

            loop {
                tokio::select! {
                    Some(update) = update_rx.recv() => {
                        match update {
                            DataUpdate::Latency(lat) => {
                                latency_proc.process_latency(lat);
                            },
                            DataUpdate::PriceLevels{snapshot, bids, asks} => {
                                if snapshot {
                                    // BIDS
                                    let cuml_bids = pricelvl_proc.add_total_volume(&bids);
                                    raw_bids.set(bids);
                                    max_total_bids.set(pricelvl_proc.get_max_volume(&cuml_bids));
                                    bid_lvls.set(pricelvl_proc.add_depths(&cuml_bids, max_total_bids()));
                                    //ASKS
                                    let cuml_asks = pricelvl_proc.add_total_volume(&asks);
                                    raw_asks.set(asks);
                                    max_total_asks.set(pricelvl_proc.get_max_volume(&cuml_asks));
                                    ask_lvls.set(pricelvl_proc.add_depths(&cuml_asks, max_total_asks()));

                                }else {
                                
                                    let mut new_bids = current_bids();
                                    new_bids.extend(bids);
                                    current_bids.set(new_bids);

                                    if current_bids().len() > ORDERBOOK_LEVELS {
                                        raw_bids.set(pricelvl_proc.apply_deltas(raw_bids(), current_bids()));
                                        let updated_bids = pricelvl_proc.add_total_volume(&raw_bids());
                                        max_total_bids.set(pricelvl_proc.get_max_volume(&updated_bids));
                                        bid_lvls.set(pricelvl_proc.add_depths(&updated_bids, max_total_bids()));
                                        // clear the state
                                        current_bids.set(vec![]);
                                    }

                                    let mut new_asks = current_asks();
                                    new_asks.extend(asks);
                                    current_asks.set(new_asks);

                                    if current_asks().len() > ORDERBOOK_LEVELS {
                                        raw_asks.set(pricelvl_proc.apply_deltas(raw_asks(), current_asks()));
                                        let updated_asks = pricelvl_proc.add_total_volume(&raw_asks());
                                        max_total_asks.set(pricelvl_proc.get_max_volume(&updated_asks));
                                        ask_lvls.set(pricelvl_proc.add_depths(&updated_asks, max_total_asks()));
                                        // clear the state
                                        current_asks.set(vec![]);
                                    }
                                }
                            }
                        }
                    }

                    //timed latency updates
                    true = latency_proc.should_update() => {
                        if let Some(latency_data) = latency_proc.get_latency_update() {
                            latency.set(latency_data);
                        }
                    }
                }
            }
    });
    */
   

    /* Update states */
    /*Without Tokio Select*/
    spawn(async move {
        // let mut latency_proc = LatencyProcessor::new();
        let mut pricelvl_proc = PriceLevelProcessor::new(ORDERBOOK_LEVELS);
        
        //TODO: set individual timeouts for different plots
        let mut data_collector = DataCollector::new(100.0, 300.0, 300.0, 50.0, 45.0);

        while let Some(update) = update_rx.recv().await {
            match update {
                DataUpdate::PlotData(plot_data) => {

                    engine_stats.push(plot_data.clone());
                    engine_stats3d.push(plot_data);

                    // if engine_stats.len() >= 2_500 && data_collector.should_update(DomElement::Latency) {
                    if data_collector.should_update(DomElement::Latency) {
                        //info!("engine stats len: {:?}", &engine_stats.len());
                        let lat = engine_stats.iter().map(|e| e.latency).collect::<Vec<i64>>();
                        latency.set(lat);

                        let lat_by_ordertype = get_latency_by_ordertype(&engine_stats, &qvals);
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


                    /*May remove but Working Version
                    if snapshot {
                        // BIDS
                        let cuml_bids = pricelvl_proc.add_total_volume(&bids);
                        raw_bids.set(bids);
                        max_total_bids.set(pricelvl_proc.get_max_volume(&cuml_bids));
                        bid_lvls.set(pricelvl_proc.add_depths(&cuml_bids, max_total_bids()));
                        //ASKS
                        let cuml_asks = pricelvl_proc.add_total_volume(&asks);
                        raw_asks.set(asks);
                        max_total_asks.set(pricelvl_proc.get_max_volume(&cuml_asks));
                        ask_lvls.set(pricelvl_proc.add_depths(&cuml_asks, max_total_asks()));

                    } else {
                        let mut new_bids = current_bids();
                        new_bids.extend(bids);
                        current_bids.set(new_bids);

                        if current_bids().len() > ORDERBOOK_LEVELS {
                            raw_bids.set(pricelvl_proc.apply_deltas(raw_bids(), current_bids()));
                            let updated_bids = pricelvl_proc.add_total_volume(&raw_bids());
                            max_total_bids.set(pricelvl_proc.get_max_volume(&updated_bids));
                            bid_lvls.set(pricelvl_proc.add_depths(&updated_bids, max_total_bids()));
                            // clear the state
                            current_bids.set(vec![]);
                        }

                        let mut new_asks = current_asks();
                        new_asks.extend(asks);
                        current_asks.set(new_asks);

                        if current_asks().len() > ORDERBOOK_LEVELS {
                            raw_asks.set(pricelvl_proc.apply_deltas(raw_asks(), current_asks()));
                            let updated_asks = pricelvl_proc.add_total_volume(&raw_asks());
                            max_total_asks.set(pricelvl_proc.get_max_volume(&updated_asks));
                            ask_lvls.set(pricelvl_proc.add_depths(&updated_asks, max_total_asks()));
                            // clear the state
                            current_asks.set(vec![]);
                        }
                    }
                    */
                },
                DataUpdate::Transactions(trades) => {
                    
                    // let mut curr_executed_orders = executed_orders.write();

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
                        /* One more working version
                        let z = &mut *fulfilled_orders.write();
                        z.extend(trades);
                        */
                        //fulfilled_orders().extend(trades);
                        /*Previous Working Ver
                        let current_fulfilled_orders = fulfilled_orders();
                        // Keep only last 25 trades
                        if current_fulfilled_orders.len() > 25 {
                            fulfilled_orders.set(current_fulfilled_orders[current_fulfilled_orders.len()-25..].to_vec());
                        }
                        */
                        /*one more working ver extn
                        if z.len() > 25 {
                            z.pop_front();
                            //fulfilled_orders().pop_front();
                        }
                        */
                        //executed_orders.clear();
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

    /*Working Version Vanilla CSS
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS },
        //document::Stylesheet { href: MAIN_CSS }
        
        div {  
            class: "header",
            h3 { "Orderbook" }
        },

        /* OB Table */
        if bid_lvls.len() > 0 && ask_lvls.len() > 0 {
        // if !FEEDKILLED() {
            OrderBookTable { bid_lvls: bid_lvls(), ask_lvls: ask_lvls() }
        },
        
        /* User Input */
        div {
            class: "btn-container" ,
            components::formDialog::Dialog { form_data },
            if FEEDKILLED() {
                button { 
                    class: "btn",
                    onclick: move|_evt| {
                    document::eval(r#"
                    const dialog = document.getElementById('favDialog');
                    dialog.showModal();
                    "#);
                },
                "Settings"
                }
            },
            button {
                class: "btn",
                onclick: move |_evt| if FEEDKILLED() { ws_client.send(Action::Start) } else { ws_client.send(Action::Stop) }, 
                if FEEDKILLED() {
                    "Start stream"
                } else {
                    "Kill Feed"
                }
            }
        },

        /*HISTOGRAM */
        LatencyHistogram { latencies: latency() }
    }
    */
    

    /* WIP1 - Working ver 
    rsx!{
        // document::Stylesheet{ href: TAILWIND_CSS },
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        // document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        div {
            class: "app-container",
            if FEEDKILLED() {
                //TODO: make a wrapper Card
                div {
                    class:"card",
                    h1 {
                        class: "card-title center",
                        "Orderbook Analysis Dashboard"
                    },
                    Tabs { mode },
                    if mode() == Mode::Simulation
                    {
                        div {
                            class: "simulation-content",
                            p {
                                class: "center",
                                style: "color: var(--text-muted)",
                                "Configure simulation settings"
                            },
                            // SettingsButton { name: "Simulation Settings", show_simualtion_settings, form_data }
                            // div {
                            //     class: "settings-panel hidden mt-4",
                            //     Dialogv2{ form_data }
                            // }
                            SettingsPanel { name: "Simulation Settings", form_data }
                        }
                    } else {
                        div {
                            class: "upload-content",
                            p {
                                class: "center",
                                style: "color: var(--text-muted)",
                                "Upload your orderbook file"
                            },
                            div {
                                class: "upload-zone mt-4",
                                label {
                                    for: "file",
                                    "Drop your file here or click to browse"
                                },
                                input {
                                    id: "file",
                                    // class: "hidden",
                                    r#type: "file",
                                    accept: ".txt,.csv"
                                }
                            }

                        }
                    }
                    //Execution button
                    div {
                        class: "center mt-6",
                        button {
                            class: "button button-primary",
                            onclick: move|_evt| {
                                // info!("sending start signal to WebSocket server")
                                ws_client.send(Action::Start)
                            },
                            "Start Execution"
                        }
                    }
                },
                //Card For mode select over
                
            } else {
                // Execution View
                div {
                    button {
                        class: "button button-danger",
                        onclick: move|_evt| {
                            ws_client.send(Action::Stop)
                        },
                        "Stop Execution"
                    },
                    OrderBookTable { bid_lvls: bid_lvls(), ask_lvls: ask_lvls() },
                    LatencyHistogram { latencies: latency() }
                }
            }
        }
    }
    */

    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS },
        div {
            class: "app-container",
            //TODO: check if FEEDKILLED check req
            if VIEW() == View::Selector {

                ModeSelector {mode, form_data },
                //Execution button
                div {
                    class: "center mt-6",
                    button {
                        class: "button button-primary",
                        onclick: move|_evt| {
                            // info!("sending start signal to WebSocket server")
                            ws_client.send(Action::Start)
                        },
                        "Start Execution"
                    }
                }
            } else if VIEW() == View::Execution {
                div {
                    class: "execution-controls",
                    button {
                        class: if FEEDKILLED() {"button"} else {"button button-danger"},
                        onclick: move |_evt| if FEEDKILLED() { ws_client.send(Action::Start) } else { ws_client.send(Action::Stop) },
                        if FEEDKILLED() {"Restart" } else { "Stop" }
                    },
                    if FEEDKILLED() {
                        button {
                            class: "button",
                            onclick: move |_evt| *VIEW.write() = View::Selector ,
                            "Change settings/mode"
                        }
                    }
                }
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
            }
        }
    }

}