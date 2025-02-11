#![allow(non_snake_case)]

use std::collections::{HashMap, VecDeque};
use dioxus::prelude::*;
use rust_decimal::Decimal;

use crate::{components::{orderbook::OrderBookTable, plot::{BarPlotCharming, HistPlotCharming}, trades::{Spread, Trades}}, ExecutedOrders};

#[component]
pub fn ExecutionView(
  bid_lvls: ReadOnlySignal<Vec<(Decimal, u64, u64, f32)>>,
  ask_lvls: ReadOnlySignal<Vec<(Decimal, u64, u64, f32)>>,
  latency: ReadOnlySignal<Vec<i64>>,
  fulfilled_orders: ReadOnlySignal<VecDeque<ExecutedOrders>>, latency_by_ordertype: ReadOnlySignal<HashMap<String, Vec<f64>>>,
  best_bid: ReadOnlySignal<Option<Decimal>>,
  best_ask: ReadOnlySignal<Option<Decimal>>,
  spread: ReadOnlySignal<Option<Decimal>>,
 ) -> Element {


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
          Spread { best_bid: best_bid(), best_ask: best_ask(), spread: spread() }
        }
      }
    },
    div {
      class: "chart-container",
      if latency.len() > 0 {
        div {
          class: "chart-card wide-chart",
          //LatencyHistogramv2 { latencies: latency() }
          // h2 { "Order latency Histogram" },
          // LatencyHistogramv2 { latency }
          HistPlotCharming { latency }
        }
      },
      if latency_by_ordertype().len() > 0 {
        div {
          class: "chart-card",
          // h2 { "Latency by Ordertype" },
          // BarPlot { latency_by_ordertype }
          BarPlotCharming { latency_by_ordertype }
        }
      },
      div {
        class: "chart-card",
        "AVL Rebalances Canvas"
      },
    }
  }
}