#![allow(non_snake_case)]

use std::collections::{BTreeMap, HashMap};
use dioxus::prelude::*;
use rust_decimal::Decimal;
use crate::components::{
  orderbook::OrderBookTable,
  plot_rt::{BarPlotCharming, HistPlotCharming, Plot3D},
  plot_st::{BarPlotCharmingCuml, HistPlotCharmingCuml, Plot3DCuml},
  trades::{Spread, Trades}
};
use crate::pages::simulator::ExecutedOrders;

#[component]
pub fn ExecutionView(
  bid_lvls: ReadOnlySignal<Vec<(Decimal, u64, u64, f32)>>,
  ask_lvls: ReadOnlySignal<Vec<(Decimal, u64, u64, f32)>>,
  latency: ReadOnlySignal<Vec<i64>>,
  fulfilled_orders: ReadOnlySignal<Vec<ExecutedOrders>>,
  latency_by_ordertype: ReadOnlySignal<HashMap<String, Vec<f64>>>,
  latency_by_avl_trade: ReadOnlySignal<BTreeMap<(i64, i64), f64>>,
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
          // h2 { "Order latency Histogram" },
          HistPlotCharming { latency }
          
        }
      },
      if latency_by_ordertype().len() > 0 {
        div {
          class: "chart-card",
          // h2 { "Latency by Ordertype" },
          BarPlotCharming { latency_by_ordertype }
        }
      },
      div {
        class: "chart-card",
        Plot3D { latency_by_avl_trade }
      },
    },
    
  }
}


#[component]
pub fn CumlExecutionView(
  cuml_latency: Vec<i64>,
  cuml_latency_by_ordertype: HashMap<String, Vec<f64>>,
  cuml_latency_by_avl_trade: BTreeMap<(i64, i64), f64>) -> Element {

  rsx!{
    div {
      class: "cumlative-chart-container",
      div {
        class: "chart-card",
        HistPlotCharmingCuml { latency: cuml_latency }
      },
      div {
        class: "cumlative-bottom-chart-container",
        div {
          // class: "cumlative-bar-plot",
          class: "chart-card",
          BarPlotCharmingCuml { latency_by_ordertype: cuml_latency_by_ordertype }
        },
        div {
          // class: "cumlative-3d-plot", 
          class: "chart-card",
          Plot3DCuml { latency_by_avl_trade: cuml_latency_by_avl_trade}
        }
      }
    }
  }
}