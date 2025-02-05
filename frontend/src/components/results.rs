#![allow(non_snake_case)]

use dioxus::prelude::*;
use rust_decimal::Decimal;

use crate::{components::{orderbook::OrderBookTable, plot::LatencyHistogramv2, trades::Trades}, ExecutedOrders};

#[component]
pub fn ExecutionView(bid_lvls: Signal<Vec<(Decimal, u64, u64, f32)>>, ask_lvls: Signal<Vec<(Decimal, u64, u64, f32)>>, latency: Signal<Vec<u128>>, fulfilled_orders: Signal<Vec<ExecutedOrders>> ) -> Element {
  rsx! {
    div {
      class: "table-container",
      div {
        class: "table-card orderbook-card",
        OrderBookTable { bid_lvls: bid_lvls(), ask_lvls: ask_lvls() }
      },
      div {
        class: "results-right-column",
        div {
          class: "table-card trades-card",
          Trades { transactions: fulfilled_orders() }
        },
        div {
          class: "table-card spread-card",
          "Spread -- ()"
        }
      }
    },
    div {
      class: "chart-container",
      div {
        class: "chart-card wide-chart",
        //LatencyHistogramv2 { latencies: latency() }
        h2 { "Order latency Histogram" },
        LatencyHistogramv2 { latency }
      },
      div {
        class: "chart-card",
        "Boxplot CANVAS"
      },
      div {
        class: "chart-card",
        "AVL Rebalances Canvas"
      },
    }
  }
}