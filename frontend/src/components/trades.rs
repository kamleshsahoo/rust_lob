#![allow(non_snake_case)]

use dioxus::prelude::*;
use rust_decimal::Decimal;
use crate::pages::simulator::ExecutedOrders;

#[component]
pub fn Trades(transactions: Vec<ExecutedOrders>) -> Element {
  
  rsx! {
    table {
      class: "trades-table",
      tbody {
        tr {
          th { scope:"col", "#AId" },
          th { scope:"col", "#PId" },
          th { scope:"col", "Price" },
          th { scope:"col", "Vol" },
        }
        for (idx, t) in transactions.iter().enumerate() {
          tr {
            key: "trade-row{idx}",
            td {"{t.aggresive_order_id}"},
            td {"{t.passive_order_id}"},
            td {"{t.price}"},
            td {"{t.volume}"},
          }
        }
      }
    }
  }
}

#[component]
pub fn Spread(best_bid: Option<Decimal>, best_ask: Option<Decimal>, spread: Option<Decimal>) -> Element {

  rsx! {
    div {
      class: "spread-container",
      // Best Bid
      if best_bid.is_some() {
        span {
          class: "metric best-buy",
          svg {
            class: "icon",
            view_box: "0 0 24 24",
            width: "24",
            height: "24",
            path {
              fill: "none",
              stroke: "currentcolor",
              stroke_linecap: "round",
              stroke_linejoin: "round",
              stroke_width: "2",
              d: "M5 12l7-7 7 7M5 19l7-7 7 7"
            }
          },
          div {
            class: "metric-content",
            label { "Best Buy" },
            span {
              class: "value",
              "{best_bid.unwrap()}"
            }
          }
        }
      }
      // Best Ask
      if best_ask.is_some() {
        span {
          class: "metric best-ask",
          svg {
            class: "icon",
            view_box: "0 0 24 24",
            width: "24",
            height: "24",
            path {
              fill: "none",
              stroke: "currentcolor",
              stroke_linecap: "round",
              stroke_linejoin: "round",
              stroke_width: "2",
              d: "M19 5l-7 7-7-7M19 12l-7 7-7-7"
            }
          },
          div {
            class: "metric-content",
            label { "Best Ask" },
            span {
              class: "value",
              "{best_ask.unwrap()}"
            }
          }
        }
      }
      // Spread
      if spread.is_some() {
        span {
          class: "metric spread",
          svg {
            class: "icon",
            view_box: "0 0 24 24",
            width: "24",
            height: "24",
            path {
              fill: "none",
              stroke: "currentcolor",
              stroke_linecap: "round",
              stroke_linejoin: "round",
              stroke_width: "2",
              d: "M8 8l4-4 4 4M8 16l4 4 4-4"
            }
          },
          div {
            class: "metric-content",
            label { "Spread" },
            span {
              class: "value",
              "{spread.unwrap()}"
            }
          }
        }
      }
    }
  }
}