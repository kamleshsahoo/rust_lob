use std::collections::HashSet;
// use dioxus::{html::g::order, logger::tracing::warn};
use rust_decimal::Decimal;

pub struct PriceLevelProcessor {
  orderbook_levels: usize
}


impl PriceLevelProcessor {
  pub fn new(orderbook_levels: usize) -> Self{
    PriceLevelProcessor {orderbook_levels}
  }

  // Check if a price level exists in current levels
  fn level_exists(&self, delta_level_price: Decimal, current_levels: &Vec<(Decimal, u64)>) -> bool {
    current_levels.iter().any(|level| level.0 == delta_level_price)
  }

  // Add a new price level to existing levels
  fn add_price_level(&self, delta_level: (Decimal, u64), mut levels: Vec<(Decimal, u64)>) -> Vec<(Decimal, u64)> {
    levels.push(delta_level);
    levels
  }

  // Remove a price level
  fn remove_price_level(&self, delta_level_price: Decimal, levels: Vec<(Decimal, u64)>) -> Vec<(Decimal, u64)> {
    levels.into_iter().filter(|lvl| lvl.0 != delta_level_price).collect::<Vec<(Decimal, u64)>>()
  }

  // Update a price level
  fn update_price_level(&self, delta_level: (Decimal, u64), levels: Vec<(Decimal, u64)>) -> Vec<(Decimal, u64)> {
    levels.into_iter().map(|lvl| if lvl.0 == delta_level.0 {
      delta_level
    } else {
      lvl
    }).collect()
  }

  pub fn get_max_volume(&self, orders: &Vec<(Decimal, u64, u64)>) -> u64{
    let max_vol = orders.iter().map(|limit| limit.2).max().unwrap_or(0);
    max_vol
  }

  pub fn apply_deltas(&self, current_levels: Vec<(Decimal, u64)>, orders: Vec<(Decimal, u64)>) -> Vec<(Decimal, u64)> {
    //let mut updated_levels = current_levels.clone();
    // const ORDERBOOK_LEVELS: usize = 25;
    
    let order_prices: HashSet<&Decimal> = orders.iter().map(|(price, _)| price).collect();
    // filter orders that were removed or fulfilled (executed)
    let filtered_current_levels = current_levels.into_iter().filter(|(price, _)| order_prices.contains(price)).collect::<Vec<_>>();
    let mut updated_levels = filtered_current_levels.clone();
    
    /* Working Ver
    for delta_level in orders {
      let (delta_level_price, _delta_level_volume) = delta_level;
      //if delta_level_volume == 0 && updated_levels.len() > ORDERBOOK_LEVELS {
      if !self.level_exists(*delta_level_price, &current_levels) && updated_levels.len() > self.orderbook_levels {
        //warn!("0 vol rcvd from engine");
        warn!("Price level removed: {:?}", delta_level_price);
        updated_levels = self.remove_price_level(*delta_level_price, updated_levels);

      } else {
          if self.level_exists(*delta_level_price, &current_levels) {
            updated_levels = self.update_price_level(*delta_level, updated_levels);
          } else {
            // If the price level doesn't exist and there are less than 25 levels, add it
            if updated_levels.len() < self.orderbook_levels {
              updated_levels = self.add_price_level(*delta_level, updated_levels);
          }
          }
      }
    }
    */

    for delta_level in orders {
      let (delta_level_price, _delta_level_volume) = delta_level;
      //if delta_level_volume == 0 && updated_levels.len() > ORDERBOOK_LEVELS {
      if self.level_exists(delta_level_price, &filtered_current_levels) {
        updated_levels = self.update_price_level(delta_level, updated_levels);
      } else {
        // If the price level doesn't exist and there are less than 25 levels, add it
        if updated_levels.len() < self.orderbook_levels {
          updated_levels = self.add_price_level(delta_level, updated_levels);
        }
      }
    }
    updated_levels
  }

  pub fn add_depths(&self, orders: &Vec<(Decimal, u64, u64)>, max_vol: u64) -> Vec<(Decimal, u64, u64, f32)> {
    orders.into_iter().map(|limit| (limit.0, limit.1, limit.2, (limit.2 as f32/max_vol as f32)*100.0)).collect::<Vec<(Decimal, u64, u64, f32)>>()
  }

  pub fn add_total_volume(&self, orders: &Vec<(Decimal, u64)>) -> Vec<(Decimal, u64, u64)> {
    orders.iter().scan(0,|state, limit| {
      *state += limit.1;
      Some((limit.0, limit.1, *state))
    }).collect::<Vec<(Decimal, u64, u64)>>()
  }
}


