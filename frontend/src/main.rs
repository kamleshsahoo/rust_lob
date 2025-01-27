use std::{collections::HashMap, sync::OnceLock};
use components::priceLvls::{addDepths, addTotalVolume, applyDeltas, getMaxVolume};
use dioxus::{logger::tracing::{info, error}, prelude::*};
use futures::{stream::SplitSink, SinkExt};
use futures_util::StreamExt;
use gloo_net::websocket::{futures::WebSocket, Message};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

mod components {
    pub mod formDialog;
    pub mod priceLvls;
}

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
  ExecutionStats (EngineStats)
}

#[derive(Debug, Deserialize)]
pub struct EngineStats {
  order_type: String,
  latency: u128,
  avl_rebalances: u64,
  executed_orders_cnt: usize
}

#[derive(Debug, Deserialize)]
pub struct ExecutedOrders {
  price: Decimal,
  volume: u64,
  aggresive_order_id: u64,
  passive_order_id: u64,
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

/*TODO: OLD version
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    Start { 
        client_name: String, 
        total_objects: Option<usize>,  // Optional, defaults to 20
        throttle_nanos: Option<u64>, // Optional, defaults to 100ns
    },
    Stop,
}
*/

#[derive(Debug, Serialize)]
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


/*Template Router
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/blog/:id")]
    Blog { id: i32 },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_SVG: Asset = asset!("/assets/header.svg");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS } document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {}
    }
}

#[component]
pub fn Hero() -> Element {
    rsx! {
        div {
            id: "hero",
            img { src: HEADER_SVG, id: "header" }
            div { id: "links",
                a { href: "https://dioxuslabs.com/learn/0.6/", "📚 Learn Dioxus" }
                a { href: "https://dioxuslabs.com/awesome", "🚀 Awesome Dioxus" }
                a { href: "https://github.com/dioxus-community/", "📡 Community Libraries" }
                a { href: "https://github.com/DioxusLabs/sdk", "⚙️ Dioxus Development Kit" }
                a { href: "https://marketplace.visualstudio.com/items?itemName=DioxusLabs.dioxus", "💫 VSCode Extension" }
                a { href: "https://discord.gg/XgGxMSkvUM", "👋 Community Discord" }
            }
        }
    }
}

/// Home page
#[component]
fn Home() -> Element {
    rsx! {
        Hero {}

    }
}

/// Blog page
#[component]
pub fn Blog(id: i32) -> Element {
    rsx! {
        div {
            id: "blog",

            // Content
            h1 { "This is blog #{id}!" }
            p { "In blog #{id}, we show how the Dioxus router works and how URL parameters can be passed as props to our route components." }

            // Navigation links
            Link {
                to: Route::Blog { id: id - 1 },
                "Previous"
            }
            span { " <---> " }
            Link {
                to: Route::Blog { id: id + 1 },
                "Next"
            }
        }
    }
}

/// Shared navbar component.
#[component]
fn Navbar() -> Element {
    rsx! {
        div {
            id: "navbar",
            Link {
                to: Route::Home {},
                "Home"
            }
            Link {
                to: Route::Blog { id: 1 },
                "Blog"
            }
        }

        Outlet::<Route> {}
    }
}
*/

static WS: GlobalSignal<Option<SplitSink<WebSocket, Message>>> = Signal::global(||None);
const ORDERBOOK_LEVELS: usize = 25;

// #[derive(Clone, Copy)]
// struct MyState {
//     bid_lvls: Vec::<(Decimal, u64)>,
// }

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    
    let WSS_URL = "ws://127.0.0.1:7575/wslob"; 
   
    let mut is_feed_killed = use_signal(||true);
    let form_data = use_signal(||HashMap::new());
    let mut bid_lvls = use_signal(||Vec::<(Decimal, u64, u64, f32)>::new());
    let mut raw_bids = use_signal(||Vec::<(Decimal, u64)>::new());
    let mut max_total_bids: Signal<u64> = use_signal(||0);
    // let state = use_context_provider(||MyState {
    //     bid_lvls: Signal::new(vec![])(),
    // });
    
    // let mut current_bids = Vec::<(Decimal, u64)>::new();

    // let u = use_effect(move || {
    //     let current_bid_lvls = bid_lvls();
    //     let updated_bids = applyDeltas(current_bid_lvls, orders);

    // });

    let ws_client = use_coroutine(move|mut rx|async move { 
        while let Some(action) = rx.next().await {
            match action {
                Action::Start => {
                    
                    let ws = WebSocket::open(WSS_URL).expect("error opening WS connection!");
                    let(mut write, mut read) = ws.split();
                    
                    let mut current_form_data = form_data();
                    info!("formdata: {:?}", current_form_data);
                    
                    let orders = current_form_data.remove("orders").and_then(|v: FormValue| Some(v.as_value().parse::<usize>().unwrap_or(10)));
                    
                    //.map_or(Some(10), |o| Some(o));
                    let unit = current_form_data.remove("units").and_then(|v| Some(v.as_value())).map_or(String::from("μs"), |f| f);

                    let conv_factor = conv_nano().get(unit.as_str()).expect("failed to get the conv factor"); 
                    
                    let throttle = current_form_data.remove("time").and_then(|v: FormValue| Some(v.as_value().parse::<u64>().unwrap_or(100) * conv_factor));
                    //.map_or(Some(1_000_000_000), |t| Some(t));

                    let mean_price = current_form_data.remove("mean_price").and_then(|v| Some(v.as_value().parse::<f64>().unwrap_or(300.0)));
                    
                    let sd_price = current_form_data.remove("sd_price").and_then(|v| Some(v.as_value().parse::<f64>().unwrap_or(50.0)));

                    let price_lvls_display = current_form_data.remove("price_lvl").and_then(|v| Some(v.as_value().parse::<bool>().unwrap_or(false)));

                    //let client_msg = ClientMessage::Start { client_name: String::from("ks"), total_objects: orders.or(Some(10)), throttle_nanos: throttle.or(Some(1_000_000_000)) };
                    let client_msg = ClientMessage::Start { total_objects: orders, throttle_nanos: throttle, mean_price, sd_price, best_price_levels: price_lvls_display };
                    
                    info!("prepped formdata: {:?}", &client_msg);

                    let start_msg =  serde_json::to_string(&client_msg).expect("error deserializing START message!"); 

                    match write.send(Message::Text(start_msg)).await {
                        Ok(_) => {
                            info!("START msg sent succ to server");
                            is_feed_killed.set(false);
                        },
                        Err(e) => error!("error {:?} occ sending START msg to server", e)
                    };

                    *WS.write() = Some(write);
                    // spawn task to handle orderbook updates from server
                    spawn(async move {
                        while let Some(Ok(server_msg)) = read.next().await {
                            //info!("**server msg: {:?}", server_msg);
                            match server_msg {
                                Message::Text(s) => {
                                    let updates = serde_json::from_str::<Vec<ServerMessage>>(&s).expect("error deserializing orderbook updates from server!");
                                   info!("*parsed server msg: {:?}", updates);
                                   for u in updates {
                                    match u {
                                        ServerMessage::PriceLevels { snapshot, bids, asks } => {
                                            if snapshot {
                                                //let b = bids.clone();
                                                let cuml_bids = addTotalVolume(&bids);
                                                raw_bids.set(bids);
                                                max_total_bids.set(getMaxVolume(&cuml_bids));
                                                bid_lvls.set(addDepths(&cuml_bids, max_total_bids()));
                                            } else {
                                                if !bids.is_empty() {
                                                    let mut current_bids = Vec::<(Decimal, u64)>::new();
                                                    current_bids.extend(bids);
                                                    if current_bids.len() > ORDERBOOK_LEVELS {
                                                        
                                                        let updated_bids = addTotalVolume(&applyDeltas(raw_bids(), &current_bids, ORDERBOOK_LEVELS));
                                                        max_total_bids.set(getMaxVolume(&updated_bids));
                                                        bid_lvls.set(addDepths(&updated_bids, max_total_bids()));
                                                        
                                                        // current_bids.clear();
                                                    }
                                                }
                                            }
                                        },
                                        ServerMessage::ExecutionStats(EngineStats { order_type, latency, avl_rebalances, executed_orders_cnt }) => {
                                            info!("execution stats:\n{:?} {:?} {:?} {:?}", order_type, latency, avl_rebalances, executed_orders_cnt);
                                        },
                                        ServerMessage::Trades(t) => {
                                            info!("executed orders: {:?}", t);
                                        }
                                    }
                                   }
                                },
                                Message::Bytes(b) => {info!("recvd bytes from server {:?}", b)}
                            }
                        }
                        is_feed_killed.set(true);
                    });
                },
                Action::Stop => {
                    if let Some(mut z) = WS.write().take() {
                        let stop_msg = serde_json::to_string(&ClientMessage::Stop).expect("error deserializing STOP message!"); 
                        match z.send(Message::Text(stop_msg)).await {
                            Ok(_) => {
                                info!("STOP msg sent succ to server");
                                is_feed_killed.set(true);                         
                            },
                            Err(e) => error!("error {:?} occ sending STOP msg to server", e)
                        };
                    }
                }
            }
        }
    });

    rsx! {
        "Hotdog!",
        components::priceLvls::buildPriceLevels { lvls: bid_lvls() },
        components::formDialog::Dialog { form_data },
        if is_feed_killed() {
            button { onclick: move|_evt| {
                document::eval(r#"
                const dialog = document.getElementById('favDialog');
                dialog.showModal();
                "#);
            },
            "Settings"
            }
        },
        button {
            onclick: move |_evt| if is_feed_killed() { ws_client.send(Action::Start) } else { ws_client.send(Action::Stop) }, 
            if is_feed_killed() {
                "Start stream"
            } else {
                "Kill Feed"
            }
        }
    }
}
