#![allow(non_snake_case)]

use dioxus::{logger::tracing::{info, warn}, prelude::*};
use rust_decimal::Decimal;

#[derive(PartialEq, Props, Clone)]
pub struct PriceProps {
    lvls: Vec<(Decimal, u64, u64, f32)>,
    reversed: bool
}

// pub fn buildPriceLevels(lvls: Vec::<(Decimal, u64, u64, f32)>) -> Element {
pub fn buildPriceLevels(props: PriceProps) -> Element {
  let lvls = props.lvls;
  let to_reverse = props.reversed; //false for Bids, true for Asks

  rsx! {
      for (idx, val) in lvls.iter().enumerate() {
        //let (size_, total_, depth_) = (val.1, val.2, val.3);
        div { 
          key: "{idx}",
          class: "row-container",
          DepthVisualizer {key: "dep{idx}", depth: val.3, reverse: to_reverse},
          PriceLevelRow {key: "pv{idx}", total: val.2, size: val.1, price: val.0, reverse: to_reverse}
        },
      }
  }
}

#[component]
pub fn TitleRow(reverse: bool) -> Element {
  if reverse {
    rsx! {
      div {
        id: "titlerow",  
        span { "PRICE" },
        span { "SIZE" },
        span { "TOTAL" }
      }
    }  
  } else {
    rsx! {
      div {
        id: "titlerow",  
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
        height: "1.25rem",
        position: "relative",
        top: "21px",
        z_index: 1,
        background_color: "#3d1e28",
        left: 0,
        margin_top: "-24px" 
      }
    }
  } else {
    rsx! {
      div {
        width: "{depth}%",
        height: "1.25rem",
        position: "relative",
        top: "21px",
        z_index: 1,
        background_color: "#113534",
        left: "{100.0-depth}%",
        margin_top: "-24px"
      }
    }
  }
}

#[component]
fn PriceLevelRow(total: u64, size:u64, price: Decimal, reverse: bool) -> Element {
  
  rsx! { 
    div {
      class: "pricerow",
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



