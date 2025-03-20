use dioxus::prelude::*;
use crate::Route;

#[component]
pub fn Home() -> Element {
  static CSS: Asset = asset!("assets/home.css");
  rsx! {
    document::Stylesheet {href: CSS},
    div {
      class: "home-page",
      section { 
        class : "hero",
        h1 { "High-Frequency Trading Orderbook Simulator" },
        p { "Experience real-time order execution and visualization at nanosecond precision. Visualize market dynamics in real time and have fun with a low-latency simulation engine" },
        Link { 
          class: "cta-button",
          to: Route::Simulator { },
          "Launch Simulator"
        }
      },
      section {
        class: "features",
        div {
          class: "feature-card",
          h3 {
            class: "feature-card-title",
            span {"⚡"} 
            "Real-Time Simulation"
          }
          p { "Run orderbook simulations with customizable parameters and watch orders execute in real-time. Visualize price levels and execution latency as they evolve." }
        },
        div {
          class: "feature-card",
          h3 { 
            class: "feature-card-title", 
            span {"📁"}
            "Custom Data Import" }
          p { "Upload your own data files for comprehensive analysis. Backtest trading strategies and track key performance metrics like liquidity and orderbook dynamics." }
        },
        div {
          class: "feature-card",
          h3 { 
            class: "feature-card-title",
            span {"🔧"} 
            "Advanced Architecture"
          }
          p { "Built on efficient data structures like AVL trees for optimal performance. Experience professional-grade order matching at nanosecond speeds." }
        }
      }
    }
  }
}