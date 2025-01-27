#![allow(non_snake_case)]

use dioxus::{logger::tracing::{info, warn}, prelude::*};
use rust_decimal::Decimal;


// #[component]
// pub fn PriceLevels(bid_lvls: Vec<(Decimal, u64, f32)>) -> Element {
//   rsx!{
//     table {  
//       tbody {  
//         for (idx, val) in bid_lvls.iter().enumerate() {
//           tr {  
//             key: "{idx}",
//             td {  "{val.0}" },
//             td {  "{val.1}" },
//             td {  "{val.2:.2}" },
//           }
//         }
//       }
//     }
//   }
// }

#[component]
pub fn buildPriceLevels(lvls: Vec::<(Decimal, u64, u64, f32)>) -> Element {
  rsx! {
      for (idx, val) in lvls.iter().enumerate() {
        DepthVisualizer {depth: val.3},
        PriceLevelRow {total: val.2, size: val.1, price: val.0},
      }
  }
}

#[component]
fn DepthVisualizer(depth: f32) -> Element {
  rsx! {
    div {  
      width: "{depth}%"
    }
  }
}

#[component]
fn PriceLevelRow(total: u64, size:u64, price: Decimal) -> Element {
  rsx! {
    div {  
      span {"{total}"},
      span {"{size}"},
      span {"{price}"},
    }
  }
}


// Check if a price level exists in current levels
fn level_exists(delta_level_price: Decimal, current_levels: &Vec<(Decimal, u64)>) -> bool {
  current_levels.iter().any(|level| level.0 == delta_level_price)
}

// Add a new price level to existing levels
fn add_price_level(delta_level: (Decimal, u64), mut levels: Vec<(Decimal, u64)>) -> Vec<(Decimal, u64)> {
  levels.push(delta_level);
  levels
}

// Remove a price level
fn remove_price_level(delta_level_price: Decimal, levels: Vec<(Decimal, u64)>) -> Vec<(Decimal, u64)> {
  levels.into_iter().filter(|lvl| lvl.0 != delta_level_price).collect::<Vec<(Decimal, u64)>>()
}

// Update a price level
fn update_price_level(delta_level: (Decimal, u64), levels: Vec<(Decimal, u64)>) -> Vec<(Decimal, u64)> {
  levels.into_iter().map(|lvl| if lvl.0 == delta_level.0 {
    delta_level
  } else {
    lvl
  }).collect()
}

pub fn getMaxVolume(orders: &Vec<(Decimal, u64, u64)>) -> u64{
  let maxVol = orders.iter().map(|limit| limit.2).max().unwrap_or(0);
  maxVol
}

pub fn applyDeltas(current_levels: Vec<(Decimal, u64)>,
orders: &Vec<(Decimal, u64)>, ORDERBOOK_LEVELS: usize) -> Vec<(Decimal, u64)> {
  let mut updated_levels = current_levels.clone();
  // const ORDERBOOK_LEVELS: usize = 25;

  for delta_level in orders {
    let (delta_level_price, _delta_level_volume) = delta_level;
    //if delta_level_volume == 0 && updated_levels.len() > ORDERBOOK_LEVELS {
    if !level_exists(*delta_level_price, &current_levels) && updated_levels.len() > ORDERBOOK_LEVELS {
      //warn!("0 vol rcvd from engine");
      warn!("Price level removed: {:?}", delta_level_price);
      updated_levels = remove_price_level(*delta_level_price, updated_levels);

    } else {
        if level_exists(*delta_level_price, &current_levels) {
          updated_levels = update_price_level(*delta_level, updated_levels);
        } else {
          // If the price level doesn't exist and there are less than 25 levels, add it
          if updated_levels.len() < ORDERBOOK_LEVELS {
            updated_levels = add_price_level(*delta_level, updated_levels);
        }
        }
    }
  }
  updated_levels
}

pub fn addDepths(orders: &Vec<(Decimal, u64, u64)>, maxVol: u64) -> Vec<(Decimal, u64, u64, f32)> {
  orders.into_iter().map(|limit| (limit.0, limit.1, limit.2, (limit.2 as f32/maxVol as f32)*100.0)).collect::<Vec<(Decimal, u64, u64, f32)>>()

}

pub fn addTotalVolume(orders: &Vec<(Decimal, u64)>) -> Vec<(Decimal, u64, u64)> {
  orders.into_iter().scan(0,|state, &limit| {
    *state += limit.1;
    Some((limit.0, limit.1, *state))
  }).collect::<Vec<(Decimal, u64, u64)>>()
}

