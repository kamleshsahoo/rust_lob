use dioxus::prelude::*;
use crate::Route;

#[component]
pub fn SimLayout() -> Element {

  rsx! {
    Outlet::<Route> {}
    SimAccordion { }
  }
}

#[component]
fn SimAccordion() -> Element {
  rsx! {
    div {
      class: "sim-accordion",
      style: "margin-bottom: 0.25em",
      details {
        summary { "Simulation parameter configuration" }
        div {
          class: "acc-param-list",
          div {
            class: "acc-param-item",
            div {class: "acc-param-name", "Orders"},
            div {
              class: "acc-param-value",
              "The number of orders included in the simulation"
              div {class: "acc-param-range", "Range: 50,000 - 1,500,000"}
          },
            
          }
          div {
            class: "acc-param-item",
            div {class: "acc-param-name", "Mean Price"},
            div {
              class: "acc-param-value",
              "The average price around which orders are generated"
              div {class: "acc-param-range", "Range: 100 - 500"}
              div {class: "acc-param-range", "Orders follow a standard normal distribution with this value as the mean."}
            }
          }
          div {
            class: "acc-param-item",
            div {class: "acc-param-name", "Price Variation (Standard Deviation)"},
            div {
              class: "acc-param-value",
              "Controls how much prices fluctuate"
              div {class: "acc-param-range", "Range: 5 - 50"}
              div {class: "acc-param-range", "Lower values create more clustered price levels, leading to a higher chance of trades."}
            }
          }
          div {
            class: "acc-param-item",
            div {class: "acc-param-name", "Probabilities"},
            div {
              class: "acc-param-value",
              "Determines how frequently each order type will be generated in the simulation"
            }
          }
          div {
            class: "acc-note-container",
            div { class: "acc-note-icon", "⚠️" }
            div { 
              class: "acc-note-text",
              strong {"Note:"}
              " CANCEL and MODIFY orders will create ADD orders as a fallback when the orderbook is depleted beyond a certain limit. This means you will see ADD orders in the plots even if the probability is set to 0."
            }
          }
        }
        Link {
          class: "acc-docs-link",
          to: Route::EngineDocs { },
          "Visit documentation for more details →"
        }
      }
      details {
        summary { "File Upload Format" }
        div {
          class: "acc-param-list",
          div {
            class: "acc-param-item",
            div {class: "acc-param-name", "Supported Format"},
            div {
              class: "acc-param-value",
              "Upload a plain text ("
             code {".txt"} 
             ") file with orders in the following format:"
            }
            pre {
              "
    ADD, <Order ID>, <Side>, <Quantity>, <Price>
    MODIFY, <Order ID>, <New Quantity>, <New Price>
    CANCEL, <Order ID>
              "
            }
            div {
              class: "acc-param-value",
              "Example:"
            }
            pre {
              "
    ADD, 1, Ask, 50, 100.2
    ADD, 2, Ask, 50, 120
    MODIFY, 2, 50, 75
    CANCEL, 1
              "
            }
          }
          div {
            class: "acc-param-item",
            div { class: "param-name", "File Handling" }
            div {
              class: "acc-param-value",
              div {
                class: "acc-sub-list",
                div {
                  class: "acc-sub-item",
                  "If a file contains invalid orders, they will be skipped, and processing will continue."
                },
                div {
                  class: "acc-sub-item",
                  "Files under " strong { "5MB" } " will display a preview table showing the first and last five parsed rows (with an ellipsis " code{"..."} " in between if necessary)."
                },
                div {
                  class: "acc-sub-item",
                  "Larger files are sent directly to the server for processing without a preview."
                },
                div {
                  class: "acc-sub-item",
                  "Once processing is complete, a summary table will display key metrics, including valid order counts, AVL tree rebalances, executed trades, and total processing time for each order type."
                },
              }
            }
          }
        }
        Link {
          class: "acc-docs-link",
          to: Route::BackDocs { },
          "For more details on file-based order processing, refer to the documentation →"
        }
      }
    }
  }
}