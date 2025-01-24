use std::{cmp::Reverse, collections::BinaryHeap, str::FromStr, time::{Duration, Instant}};
use chrono::{DateTime, Utc};
use rand::{rngs::ThreadRng, thread_rng};
use rand_distr::{Bernoulli, Distribution, Normal, Uniform};
use rust_decimal::{prelude::ToPrimitive, Decimal};

use crate::engine::orderbook::{Arena, BidOrAsk, ExecutedOrders};

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

#[derive(Debug)]
enum OrderType {
  Add,
  Update,
  Delete,
}

// #[derive(Debug)]
// struct ObDelta {
//   order_type: OrderType,
//   id: u64,
//   side: BidOrAsk,
//   price: Decimal,
//   qty: u64,
//   timestamp: DateTime<Utc>
// }

#[derive(Debug)]
enum ServerMessage {
  PriceLevels { bids: Vec<(Decimal, u64)>, asks: Vec<(Decimal, u64)> },
  Trades(Vec<ExecutedOrders>),
  EngineStats(Vec<ExecutionStats>)
}

#[derive(Debug, Clone)]
struct ExecutionStats {
  order_type: String,
  latency: u128,
  avl_rebalances: u64,
  executed_orders_cnt: u64
}

pub struct Simulator {
  // symbol: String,
  // sequence_number: u64,
  book : Arena,
  execution_stats: Vec<ExecutionStats>,
  rng: ThreadRng,
  order_id: u64,
  mean_limit_price: f64,
  sd_limit_price: f64,
  order_type_dist: Uniform<f32>,
  order_type_cuml_probs: Vec<f32>,
  price_dist: Normal<f64>,
  qty_dist: Uniform<u64>,
  side_dist: Bernoulli,
  price_levels_rate_limiter: RateLimiter,
  executed_orders_rate_limiter: RateLimiter,
  engine_stats_rate_limiter: RateLimiter
}

impl Simulator {
  pub fn new(mean_price: f64, sd_price: f64) -> Self {
    //TODO: could take indvidual these probs as input
    let action_probs = vec![0.0, 0.4, 0.6]; // ADD, CANCEL, MODIFY

    Simulator {
      book: Arena::new(),
      execution_stats: Vec::new(),
      rng: thread_rng(),
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
      price_levels_rate_limiter: RateLimiter::new(Duration::from_millis(100)),
      executed_orders_rate_limiter: RateLimiter::new(Duration::from_millis(500)),
      engine_stats_rate_limiter: RateLimiter::new(Duration::from_millis(100))
    }
  }

  fn sample_price_levels(&self) {
    todo!()
  }

  fn add_limit(&mut self) {
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
    self.execution_stats.push(ExecutionStats { order_type: String::from("ADD"), latency: duration, avl_rebalances: self.book.avl_rebalances, executed_orders_cnt: self.book.executed_orders_count });
    
    self.order_id += 1;
  }

  fn cancel_limit(&mut self) {
    match self.book.get_random_order_id() {
      None => self.add_limit(),
      Some(order_id) => {
        let start = Instant::now();
        self.book.cancel_limit_order(*order_id);
        let duration = start.elapsed().as_nanos();
        self.execution_stats.push(ExecutionStats { order_type: String::from("CANCEL"), latency: duration, avl_rebalances: self.book.avl_rebalances, executed_orders_cnt: self.book.executed_orders_count });
      }
    }
  }

  fn modify_limit(&mut self) {
    //TODO: highest buy checks req or not as we pre-seed
    let highest_buy = self.book.highest_buy.unwrap().to_f64().unwrap();
    let price_distr = Normal::new(highest_buy, self.sd_limit_price).expect("error creating a normal dist for modify limit!");

    match self.book.get_random_order_id() {
      None => self.add_limit(),
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
        self.execution_stats.push(ExecutionStats { order_type: String::from("MODIFY"), latency: duration, avl_rebalances: self.book.avl_rebalances, executed_orders_cnt: self.book.executed_orders_count });
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

  fn generate_orders(&mut self) {
    
    
    let rand_num = self.order_type_dist.sample(&mut self.rng);

    match self.order_type_cuml_probs.iter().position(|cumprob| rand_num <= *cumprob).expect("error getting order type idx!") {
      0 => self.add_limit(),
      1 => self.cancel_limit(),
      2 => self.modify_limit(),
      _ => panic!("error choosing a order type in generate_orders()!")
    };
  } 

  fn create_price_levels(&self, n_level: usize) -> ServerMessage {
    //TODO: check if we can make this efficient

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

  pub fn generate_updates(&mut self) -> Vec<ServerMessage>{
    
    let mut messages = Vec::new();

    // generate and process the orders
    self.generate_orders();

    // sending top `n` price levels 
    if self.price_levels_rate_limiter.should_update() {
      let price_levels = self.create_price_levels(25);
      messages.push(price_levels);
    }

    //TODO: this needs to be sent everytime, keep track of which order stats were sent last time
    if self.engine_stats_rate_limiter.should_update() {
      let engine_stats = self.execution_stats.clone();
      messages.push(ServerMessage::EngineStats(engine_stats));
    }

    //TODO: here too keep track of which trades were sent last time
    if self.executed_orders_rate_limiter.should_update() {
      let executed_orders = self.book.executed_orders.clone();
      if !executed_orders.is_empty() {
        messages.push(ServerMessage::Trades(executed_orders));
      }
    }

    messages
  }
}