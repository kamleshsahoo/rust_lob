use std::{str::FromStr, time::Instant};
// use chrono::{DateTime, Utc};
use rand::{rngs::StdRng, SeedableRng};
use rand_distr::{Bernoulli, Distribution, Normal, Uniform};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde::Serialize;

use crate::engine::orderbook::{Arena, BidOrAsk, ExecutedOrders};

#[derive(Debug, Serialize)]
// #[serde(tag = "type")]
pub enum ServerMessage {
  PriceLevels { snapshot: bool, bids: Vec<(Decimal, u64)>, asks: Vec<(Decimal, u64)> },
  Trades (Vec<ExecutedOrders>),
  ExecutionStats (EngineStats),
  BestLevels {best_buy: Option<Decimal>, best_sell: Option<Decimal>},
  Completed
}

#[derive(Debug, Serialize, Clone)]
pub struct EngineStats {
  pub order_type: String,
  pub latency: i64,
  pub avl_rebalances: i64,
  pub executed_orders_cnt: usize
}

pub struct Simulator {
  // symbol: String,
  // sequence_number: u64,
  pub book : Arena,
  engine_stats: Vec<EngineStats>,
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
      rng: StdRng::from_os_rng(),
      order_id: 1,
      mean_limit_price: mean_price,
      sd_limit_price: sd_price,
      order_type_dist: Uniform::new(0.0, 1.0).expect("error creating uniform dist for order type"),
      order_type_cuml_probs: action_probs.into_iter().scan(0.0, |acc, x| { 
        *acc += x;
        Some(*acc)
      }).collect(),
      price_dist: Normal::new(mean_price, sd_price).expect("error creating a normal distribution"),
      qty_dist: Uniform::new(1, 1000).expect("error creating uniform dist for shares/qty"),
      side_dist: Bernoulli::new(0.5).expect("error creating bernoulii distr"),
      executed_orders_offset: 0,
    }
  }

  fn create_add_limit(&mut self) {
    //println!("**ADD");
    let shares = self.qty_dist.sample(&mut self.rng);
    let side = self.side_dist.sample(&mut self.rng);

    let mut price;
    let bid_or_ask;

    if side {
      bid_or_ask = BidOrAsk::Bid;
      let lowest_sell = self.book.lowest_sell.unwrap().to_f64().unwrap();
      loop {
        price = self.price_dist.sample(&mut self.rng);
        if !(price >= lowest_sell) { 
          break;
        }
      };
    } else {
      bid_or_ask = BidOrAsk::Ask;
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
    self.engine_stats.push(EngineStats { order_type: String::from("ADD"), latency: duration as i64, avl_rebalances: self.book.avl_rebalances as i64, executed_orders_cnt: self.book.executed_orders_count });
    
    self.order_id += 1;
  }

  fn create_cancel_limit(&mut self) {
    match self.book.get_random_order_id() {
      None => self.create_add_limit(),
      Some(order_id) => {
        let start = Instant::now();
        self.book.cancel_limit_order(*order_id);
        let duration = start.elapsed().as_nanos();
        self.engine_stats.push(EngineStats { order_type: String::from("CANCEL"), latency: duration as i64, avl_rebalances: self.book.avl_rebalances as i64, executed_orders_cnt: self.book.executed_orders_count });
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
        self.engine_stats.push(EngineStats { order_type: String::from("MODIFY"), latency: duration as i64, avl_rebalances: self.book.avl_rebalances as i64, executed_orders_cnt: self.book.executed_orders_count });
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
        //println!("inserting ADD with order id: {:?}", self.order_id);
        self.create_add_limit()
      },
      1 => {
        //println!("CANCEL trigg. current order id: {:?}", self.order_id);
        self.create_cancel_limit()},
      2 => {
        //println!("MODIFY trigg. curr order id: {:?}", self.order_id);
        self.create_modify_limit()
      },
      _ => panic!("error choosing a order type in generate_orders()!")
    };
  } 

  pub fn get_snapshot(&self) -> Vec<ServerMessage> {
    vec![ServerMessage::PriceLevels { snapshot: true, bids: self.book.get_top_n_bids(20), asks: (self.book.get_top_n_asks(20)) }]
  }
  
  pub fn generate_updates(&mut self, idx: usize) -> Vec<ServerMessage>{
    
    let mut messages = Vec::new();
    // always send the engine stats
    let engine_stat = self.engine_stats.get(idx).expect("each order should have a execution stat!").clone();
    messages.push(ServerMessage::ExecutionStats(engine_stat));
    
    // sending top `n=1000` price levels 
    // NOTE: bids or asks may be empty vectors
    if (idx+1) % 100 == 0 {
      let price_levels = ServerMessage::PriceLevels { snapshot: false, bids: self.book.get_top_n_bids(1_000), asks: (self.book.get_top_n_asks(1_000)) }; 
      messages.push(price_levels);
    }

    if idx % 100 == 0 {
      messages.push(ServerMessage::BestLevels { best_buy: self.book.highest_buy, best_sell: self.book.lowest_sell });
    }

    if let Some(trades) = self.book.get_executed_orders(&mut self.executed_orders_offset) {
      messages.push(ServerMessage::Trades(trades));
    }  
    messages
  }
}