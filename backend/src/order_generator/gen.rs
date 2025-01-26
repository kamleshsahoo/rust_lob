use std::{str::FromStr, time::Instant};
// use chrono::{DateTime, Utc};
use rand::{rngs::StdRng, SeedableRng};
use rand_distr::{Bernoulli, Distribution, Normal, Uniform};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde::Serialize;

use crate::engine::orderbook::{Arena, BidOrAsk, ExecutedOrders};

/* TODO: May Remove
struct RateLimiter {
  last_update: Instant,
  update_interval: Duration
}

impl RateLimiter {
  fn new(update_frequency: Duration) -> Self {
    RateLimiter { last_update: Instant::now(), update_interval: update_frequency }
  }

  fn should_update(&mut self) -> bool {
    let now = Instant::now();
    if now.duration_since(self.last_update) >= self.update_interval {
      self.last_update = now;
      true
    } else {
      false
    }
  }
}
*/

/*TODO: remove at end
#[derive(Debug)]
enum OrderType {
  Add,
  Update,
  Delete,
}

#[derive(Debug)]
struct ObDelta {
  order_type: OrderType,
  id: u64,
  side: BidOrAsk,
  price: Decimal,
  qty: u64,
  timestamp: DateTime<Utc>
}
*/

#[derive(Debug, Serialize)]
// #[serde(tag = "type")]
pub enum ServerMessage {
  PriceLevels { bids: Vec<(Decimal, u64)>, asks: Vec<(Decimal, u64)> },
  Trades (Vec<ExecutedOrders>),
  // EngineStats(Vec<EngineStats>)
  ExecutionStats (EngineStats)
}

#[derive(Debug, Serialize, Clone)]
pub struct EngineStats {
  order_type: String,
  latency: u128,
  avl_rebalances: u64,
  executed_orders_cnt: usize
}

pub struct Simulator {
  // symbol: String,
  // sequence_number: u64,
  book : Arena,
  engine_stats: Vec<EngineStats>,
  //pub engine_stats_offset: usize, // tracks the last sent update to the server
  //rng: ThreadRng,
  rng: StdRng,
  order_id: u64,
  mean_limit_price: f64,
  sd_limit_price: f64,
  order_type_dist: Uniform<f32>,
  order_type_cuml_probs: Vec<f32>,
  price_dist: Normal<f64>,
  qty_dist: Uniform<u64>,
  side_dist: Bernoulli,
  executed_orders_offset: usize,
}

impl Simulator {
  pub fn new(mean_price: f64, sd_price: f64, best_price_lvls: bool) -> Self {
    //TODO: could take indvidual these probs as input
    let action_probs = vec![0.0, 0.4, 0.6]; // ADD, CANCEL, MODIFY

    Simulator {
      book: Arena::new(best_price_lvls),
      engine_stats: Vec::new(),
      //engine_stats_offset: 0,
      //rng: thread_rng(),
      rng: StdRng::from_entropy(),
      order_id: 1,
      mean_limit_price: mean_price,
      sd_limit_price: sd_price,
      order_type_dist: Uniform::new(0.0, 1.0),
      order_type_cuml_probs: action_probs.into_iter().scan(0.0, |acc, x| { 
        *acc += x;
        Some(*acc)
      }).collect(),
      price_dist: Normal::new(mean_price, sd_price).expect("error creating a normal distribution"),
      qty_dist: Uniform::new(1, 1000),
      side_dist: Bernoulli::new(0.5).expect("error creating bernoulii distr"),
      executed_orders_offset: 0,
    }
  }

  fn create_add_limit(&mut self) {
    println!("**ADD");
    let shares = self.qty_dist.sample(&mut self.rng);
    let side = self.side_dist.sample(&mut self.rng);

    let mut price;
    let bid_or_ask;

    if side {
      bid_or_ask = BidOrAsk::Bid;
      // bid_or_ask_str = "Bid";
      let lowest_sell = self.book.lowest_sell.unwrap().to_f64().unwrap();
      loop {
        price = self.price_dist.sample(&mut self.rng);
        if !(price >= lowest_sell) { 
          break;
        }
      };
    } else {
      bid_or_ask = BidOrAsk::Ask;
      // bid_or_ask_str = "Ask";
      let highest_buy = self.book.highest_buy.unwrap().to_f64().unwrap();
      loop {
        price = self.price_dist.sample(&mut self.rng);
        if !(price <= highest_buy) {
          break;
        }
      }
    }

    let price_string = format!("{:.2}", price);
    let start = Instant::now();
    self.book.add_limit_order(self.order_id, bid_or_ask, shares, Decimal::from_str(&price_string).expect("parsing price string to decimal failed"));
    let duration = start.elapsed().as_nanos();
    self.engine_stats.push(EngineStats { order_type: String::from("ADD"), latency: duration, avl_rebalances: self.book.avl_rebalances, executed_orders_cnt: self.book.executed_orders_count });
    
    self.order_id += 1;
  }

  fn create_cancel_limit(&mut self) {
    match self.book.get_random_order_id() {
      None => self.create_add_limit(),
      Some(order_id) => {
        let start = Instant::now();
        self.book.cancel_limit_order(*order_id);
        let duration = start.elapsed().as_nanos();
        self.engine_stats.push(EngineStats { order_type: String::from("CANCEL"), latency: duration, avl_rebalances: self.book.avl_rebalances, executed_orders_cnt: self.book.executed_orders_count });
      }
    }
  }

  fn create_modify_limit(&mut self) {
    //TODO: highest buy checks req or not as we pre-seed
    let highest_buy = self.book.highest_buy.unwrap().to_f64().unwrap();
    let price_distr = Normal::new(highest_buy, self.sd_limit_price).expect("error creating a normal dist for modify limit!");

    match self.book.get_random_order_id() {
      None => self.create_add_limit(),
      Some(order_id) => {
        let order = self.book.orders.get(order_id).expect("order should exist after the checks!");
        let shares = self.qty_dist.sample(&mut self.rng);
        let mut price;

        match order.bid_or_ask {
          BidOrAsk::Bid => {
            let lowest_sell = self.book.lowest_sell.unwrap().to_f64().unwrap();
            loop {
              price = price_distr.sample(&mut self.rng);
              if !(price >= lowest_sell) { 
                break;
              }
            };
          },
          BidOrAsk::Ask => {
            loop { 
              price = price_distr.sample(&mut self.rng);
              if !(price <= highest_buy) {
                break;
              }
            }
          }
        }
        let price_string = format!("{:.2}", price);
        let start = Instant::now();
        self.book.modify_limit_order(*order_id, shares, Decimal::from_str(&price_string).expect("parsing price string to decimal failed"));
        let duration = start.elapsed().as_nanos();
        self.engine_stats.push(EngineStats { order_type: String::from("MODIFY"), latency: duration, avl_rebalances: self.book.avl_rebalances, executed_orders_cnt: self.book.executed_orders_count });
      }
    }
  }

  pub fn seed_orderbook(&mut self, n: u64) {
    
    // seed the orderbook with `n` ADD Limit orders
    for i in 1..=n {
      let shares = self.qty_dist.sample(&mut self.rng);
      let limit_price = self.price_dist.sample(&mut self.rng);
      let price_string = format!("{:.2}", limit_price);
      // Initially all bids < mean price and asks >= mean price
      let bid_or_ask = if limit_price < self.mean_limit_price {BidOrAsk::Bid} else {BidOrAsk::Ask}; 

      self.book.add_limit_order(i, bid_or_ask, shares, Decimal::from_str(&price_string).expect("parsing price string to decimal failed"));
    }
    self.order_id = n+1;
  }

  pub fn generate_orders(&mut self) {
    
    let rand_num = self.order_type_dist.sample(&mut self.rng);

    match self.order_type_cuml_probs.iter().position(|cumprob| rand_num <= *cumprob).expect("error getting order type idx!") {
      0 => {
        println!("inserting ADD with order id: {:?}", self.order_id);
        self.create_add_limit()
      },
      1 => {
        println!("CANCEL trigg. current order id: {:?}", self.order_id);
        self.create_cancel_limit()},
      2 => {
        println!("MODIFY trigg. curr order id: {:?}", self.order_id);
        self.create_modify_limit()
      },
      _ => panic!("error choosing a order type in generate_orders()!")
    };
  } 

  /*TODO: Remove this
  fn create_price_levels(&self, n_level: usize) -> ServerMessage {
    // Sorted highest to lowest buy, i.e descending
    let bid_levels = self.book.buy_limits.iter()
                                                      .map(|(&k,v)| (k, v.total_volume))
                                                      .map(Reverse)
                                                      .collect::<BinaryHeap<_>>()
                                                      .into_sorted_vec()
                                                      .into_iter()
                                                      .take(n_level)
                                                      .map(|Reverse((k,v))| (k,v))
                                                      .collect::<Vec<_>>();
    // Sorted lowest to highest sell, i.e ascending
    let ask_levels = self.book.sell_limits.iter()
                                                      .map(|(&k,v)| (k, v.total_volume))
                                                      .collect::<BinaryHeap<_>>()
                                                      .into_sorted_vec()
                                                      .into_iter()
                                                      .take(n_level)
                                                      .collect::<Vec<_>>();
    
    ServerMessage::PriceLevels { bids: bid_levels, asks: ask_levels }
  }
  */

  /*NOTE: may remove this ver
  pub fn generate_updates(&mut self) -> Vec<ServerMessage>{
    
    let mut messages = Vec::new();
    // sending top `n` price levels 
    //TODO: check if we can make this efficient
    if self.price_levels_rate_limiter.should_update() {
      let price_levels = self.create_price_levels(25);
      messages.push(price_levels);
    }

    //TODO: the vec can be empty. need to handle here?
    if self.engine_stats_rate_limiter.should_update() {
      let engine_stats = self.get_engine_stats();
      messages.push(ServerMessage::EngineStats(engine_stats));
    }

    //TODO: the vec can be empty. need to handle here?
    if self.executed_orders_rate_limiter.should_update() {
      let executed_orders = self.book.get_executed_orders();
      messages.push(ServerMessage::Trades(executed_orders));
      // if !executed_orders.is_empty() {
      //   messages.push(ServerMessage::Trades(executed_orders));
      // }
    }

    messages
  }
  */

  pub fn generate_updates(&mut self, idx: usize) -> Vec<ServerMessage>{
    
    let mut messages = Vec::new();

    // sending top `n=25` price levels 
    // NOTE: bids or asks may be empty vectors
    if idx%100 == 0 {
      let price_levels = ServerMessage::PriceLevels { bids: self.book.get_top_n_bids(25), asks: (self.book.get_top_n_asks(25)) }; 
      messages.push(price_levels);
    }

    // we always send the engine stats
    let engine_stat = self.engine_stats.get(idx).expect("each order should have a execution stat!").clone();
    messages.push(ServerMessage::ExecutionStats(engine_stat));

    if let Some(trades) = self.book.get_executed_orders(&mut self.executed_orders_offset) {
      messages.push(ServerMessage::Trades(trades));
    }

    messages
  }
}