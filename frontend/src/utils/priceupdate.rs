#![allow(non_snake_case)]

use std::collections::HashMap;
use dioxus::signals::{Signal, Writable};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use crate::pages::simulator::ORDERBOOK_LEVELS;

pub struct PriceLevelProcessor { }

impl PriceLevelProcessor {
  pub fn new() -> Self{
    PriceLevelProcessor {}
  }

  fn get_max_volume(&self, orders: &Vec<(Decimal, u64, u64)>) -> u64{
    let max_vol = orders.iter().map(|limit| limit.2).max().unwrap_or(0);
    max_vol
  }

  fn apply_deltas(&self, current_levels: Vec<(Decimal, u64)>, orders: Vec<(Decimal, u64)>, isAsk: bool) -> Vec<(Decimal, u64)> {
    //let mut updated_levels = current_levels.clone();
    let order_map: HashMap<Decimal, u64> = orders.into_iter().collect();
    // Keep only existing levels that are still in orders and update their volumes
    let mut updated_levels: Vec<(Decimal, u64)> = current_levels
    .into_iter()
    .filter_map(|(price, _)| {
      order_map
      .get(&price)
      .map(|&volume| (price, volume)) }
    ).collect();

    // Add new levels that weren't in current_levels
    for (price, volume) in order_map.iter() {
      if !updated_levels.iter().any(|(p, _)| p == price) 
      && updated_levels.len() < ORDERBOOK_LEVELS { 
        updated_levels.push((*price, *volume));
      }
    }

    if isAsk {
      updated_levels.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap());
    } else {
      updated_levels.sort_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap());
    }

    updated_levels.drain(ORDERBOOK_LEVELS..);

    assert!(updated_levels.len() <= ORDERBOOK_LEVELS, "apply deltas gave {:?} price levels", updated_levels.len());

    updated_levels
  }

  fn add_depths(&self, orders: &Vec<(Decimal, u64, u64)>, max_vol: u64) -> Vec<(Decimal, u64, u64, f32)> {
    orders.into_iter().map(|limit| (limit.0, limit.1, limit.2, (limit.2 as f32/max_vol as f32)*100.0)).collect::<Vec<(Decimal, u64, u64, f32)>>()
  }

  fn add_total_volume(&self, orders: &Vec<(Decimal, u64)>) -> Vec<(Decimal, u64, u64)> {
    orders.iter().scan(0,|state, limit| {
      *state += limit.1;
      Some((limit.0, limit.1, *state))
    }).collect::<Vec<(Decimal, u64, u64)>>()
  }

  fn group_by_ticket_size(&self, levels: &Vec<(Decimal, u64)>, interval: Decimal) -> Vec<(Decimal, u64)> {
    // input-> vec![(1000, 100), (1000, 175), (900, 80)]
    // output-> vec![(1000, 275), (900, 80)]
    fn round_to_nearest(value: Decimal, interval: Decimal) -> Decimal {
      interval * (value/interval).floor()
    }
    
    let rounded_lvls = levels.iter().map(|lvl| (round_to_nearest(lvl.0, interval), lvl.1));

    let mut seen_indices: HashMap<Decimal, usize> = HashMap::new();
    let mut result: Vec<(Decimal, u64)> = vec![];

    for (key, value) in rounded_lvls {
      if let Some(&idx) = seen_indices.get(&key) {
        result[idx].1 += value;
      } else {
        seen_indices.insert(key, result.len());
        result.push((key, value));
      }
    }
    result
  }

  pub fn updater(&mut self, snapshot: bool, bids: Vec<(Decimal, u64)>, asks: Vec<(Decimal, u64)>, mut raw_bids: Signal<Vec<(Decimal, u64)>>, mut raw_asks: Signal<Vec<(Decimal, u64)>>, mut bid_lvls: Signal<Vec<(Decimal, u64, u64, f32)>>, mut ask_lvls: Signal<Vec<(Decimal, u64, u64, f32)>>) {
    // TODO: take this from ui
    let ticket_size = dec!(0.1);

    if snapshot {
      //BIDS
      //let cuml_bids = self.add_total_volume(&bids);
      let cuml_bids = self.add_total_volume(&self.group_by_ticket_size(&bids, ticket_size));
      raw_bids.set(bids);
      //max_total_bids.set(self.get_max_volume(&cuml_bids));
      let max_total_bids = self.get_max_volume(&cuml_bids);
      bid_lvls.set(self.add_depths(&cuml_bids, max_total_bids));

      //ASKS
      // let cuml_asks = self.add_total_volume(&asks);
      let cuml_asks = self.add_total_volume(&self.group_by_ticket_size(&asks, ticket_size));
      raw_asks.set(asks);
      // max_total_asks.set(self.get_max_volume(&cuml_asks));
      let max_total_asks = self.get_max_volume(&cuml_asks);
      ask_lvls.set(self.add_depths(&cuml_asks, max_total_asks));
    } else {
      //TODO: may remove this in prod 
      assert!(bids.len() == 1_000 && asks.len() == 1_000, "bids/asks length should be 100. found bids: {}, asks: {}", bids.len(), asks.len());
      //BIDS
      let grouped_current_bids = self.group_by_ticket_size(&bids, ticket_size);
      //raw_bids.set(self.apply_deltas(raw_bids(), bids));
      raw_bids.set(bids);
      //let updated_bids = self.add_total_volume(&(raw_bids)());
      let updated_bids = self.add_total_volume(&self.apply_deltas(self.group_by_ticket_size(&raw_bids(), ticket_size), grouped_current_bids, false));
      //max_total_bids.set(self.get_max_volume(&updated_bids));
      let max_total_bids = self.get_max_volume(&updated_bids);
      bid_lvls.set(self.add_depths(&updated_bids, max_total_bids));

      //ASKS
      let grouped_current_asks = self.group_by_ticket_size(&asks, ticket_size);
      // raw_asks.set(self.apply_deltas(raw_asks(), asks));
      raw_asks.set(asks);
      //let updated_asks = self.add_total_volume(&(raw_asks)());
      let updated_asks = self.add_total_volume(&self.apply_deltas(self.group_by_ticket_size(&raw_asks(), ticket_size), grouped_current_asks, true));
      //max_total_asks.set(self.get_max_volume(&updated_asks));
      let max_total_asks = self.get_max_volume(&updated_asks);
      ask_lvls.set(self.add_depths(&updated_asks, max_total_asks));
    }
  }
}


