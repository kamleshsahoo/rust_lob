#![allow(non_snake_case)]

use dioxus::prelude::*;
use rust_decimal::Decimal;

#[component]
pub fn OrderBookTable(bid_lvls: Vec<(Decimal, u64, u64, f32)>, ask_lvls: Vec<(Decimal, u64, u64, f32)>) -> Element {
  //NOTE:`reverse`=false for Bids, true for Asks
  
  rsx! {
      div {
        class: "orderbook-container",
        div {
          class: "orderbook-table-container",
          TitleRow { reverse: false },
          for (idx, val) in bid_lvls.iter().enumerate() {
            div { 
              key: "{idx}",
              class: "orderbook-row-container",
              DepthVisualizer {key: "dep{idx}", depth: val.3, reverse: false},
              PriceLevelRow {key: "pv{idx}", total: val.2, size: val.1, price: val.0, reverse: false}
            },
          }
        },
        div {
          class: "orderbook-table-container",
          TitleRow { reverse: true },
          for (idx, val) in ask_lvls.iter().enumerate() {
            div { 
              key: "{idx}",
              class: "orderbook-row-container",
              DepthVisualizer {key: "dep{idx}", depth: val.3, reverse: true},
              PriceLevelRow {key: "pv{idx}", total: val.2, size: val.1, price: val.0, reverse: true}
            },
          }
        }
      }
  }
}

#[component]
fn TitleRow(reverse: bool) -> Element {
  if reverse {
    rsx! {
      div {
        id: "orderbook-titlerow",  
        span { "PRICE" },
        span { "SIZE" },
        span { "TOTAL" }
      }
    }  
  } else {
    rsx! {
      div {
        id: "orderbook-titlerow",  
        span { "TOTAL" },
        span { "SIZE" },
        span { "PRICE" }
      }
    }
  }
}

#[component]
fn DepthVisualizer(depth: f32, reverse: bool) -> Element {
  if reverse {
    rsx! {
      div {
        width: "{depth}%",
        height: "1.2em",
        position: "relative",
        top: "22px",
        z_index: 1,
        background_color: "#3d1e28",
        left: 0,
        margin_top: "-25px" 
      }
    }
  } else {
    rsx! {
      div {
        width: "{depth}%",
        height: "1.2em",
        position: "relative",
        top: "22px",
        z_index: 1,
        background_color: "#113534",
        left: "{100.0-depth}%",
        margin_top: "-25px"
      }
    }
  }
}

#[component]
fn PriceLevelRow(total: u64, size:u64, price: Decimal, reverse: bool) -> Element {
  
  rsx! { 
    div {
      class: "orderbook-pricerow",
      if reverse {
          span {class: "price-ask", "{price}"},
          span {"{size}"},
          span {"{total}"}
        
      } else {
          span {"{total}"},
          span {"{size}"},
          span {class: "price-bid", "{price}"},
        
      }
    }
  }
}



