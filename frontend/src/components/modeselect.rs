#![allow(non_snake_case)]

use std::{fmt, collections::HashMap, str::FromStr, sync::Arc};
use dioxus::prelude::*;
use dioxus::html::FileEngine;
use dioxus::logger::tracing::{info, warn};
use rust_decimal::Decimal;
use crate::components::formDialog::Dialog;
use crate::pages::simulator::Mode;

struct PreviewRow {
  row_id: String,
  ordertype: String,
  order_id: String,
  side: String,
  shares: String,
  price: String,
}

#[derive(Debug, Clone, PartialEq)]
enum BidOrAsk {
  Bid,
  Ask,
}

impl fmt::Display for BidOrAsk {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
        Self::Bid => write!(f, "BID"),
        Self::Ask => write!(f, "ASK"),
    }
  }
}

impl FromStr for BidOrAsk {
  type Err = ParseError;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
      match s.to_lowercase().as_str() {
        "bid" => Ok(BidOrAsk::Bid),
        "ask" => Ok(BidOrAsk::Ask),
        _ => Err(ParseError::InvalidBidorAsk(s.to_string())) 
      }
  }
}

#[derive(Debug, Clone, PartialEq)]
enum FileUploadOrderType {
  Add {
    id: u64,
    side: BidOrAsk,
    shares: u64,
    price: Decimal
  },
  Modify {
    id: u64,
    shares: u64,
    price: Decimal
  },
  Cancel {
    id: u64,
  },
}

#[derive(Debug)]
enum ParseError {
  InvalidBidorAsk(String),
  InvalidOrderType(String),
  InvalidOrderFormat(String),
  Empty
}

impl fmt::Display for ParseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidBidorAsk(bid_or_ask) => {
        write!(f, "Invalid bid/ask string: {}", bid_or_ask)
      },
      Self::InvalidOrderType(order_type) => {
        write!(f, "Invalid order type string: {}", order_type)
      },
      Self::InvalidOrderFormat(order) => {
        write!(f, "Invalid {} order format", order)
      },
      Self::Empty => {
        write!(f, "Empty order line in file")
      }
    }
  }
}

impl std::error::Error for ParseError {}

struct FileUploadOrder {
  order: FileUploadOrderType
}

impl FileUploadOrder {
  fn parse(line: &str) -> Result<Self, ParseError> {
    let parts: Vec<&str> = line.split(|c| c == ',').map(|s| s.trim()).collect();

    let order_type = match parts.get(0).map(|s| s.to_uppercase()) {
        Some(s) => s,
        None => return Err(ParseError::Empty)
    };

    let order = match order_type.as_str() {
      "ADD" => {
        if parts.len() != 5 {
          return Err(ParseError::InvalidOrderFormat("ADD".to_string()));
        }
        FileUploadOrderType::Add {
          id: parts[1].parse().expect("parsing id to u64 failed"),
          side: BidOrAsk::from_str(parts[2])?,
          shares: parts[3].parse().expect("parsing shares to u64 failed"),
          price: Decimal::from_str(parts[4]).expect("parsing price to Decimal failed")
        }
      },
      "MODIFY" => {
        if parts.len() != 4 {
          return Err(ParseError::InvalidOrderFormat("MODIFY".to_string()));
        }
        FileUploadOrderType::Modify { 
          id: parts[1].parse().expect("parsing id to u64 failed"),
          shares: parts[2].parse().expect("parsing shares to u64 failed"),
          price: Decimal::from_str(parts[3]).expect("parsing price to Decimal failed")
        }
      },
      "CANCEL" => {
        if parts.len() != 2 {
          return Err(ParseError::InvalidOrderFormat("CANCEL".to_string()));
        }
        FileUploadOrderType::Cancel { id: parts[1].parse().expect("parsing id to u64 failed") }
      },
      _ => return Err(ParseError::InvalidOrderType(order_type)),
    };
    Ok(FileUploadOrder {order})
  }
}

// struct UploadedFile {
//   name: String,
//   contents: String,
// }

#[component]
pub fn ModeSelector(mut mode: Signal<Mode>, mut form_data: Signal<HashMap<String, FormValue>>) -> Element {

  // let mut files_uploaded = use_signal(|| Vec::new() as Vec<UploadedFile>);
  
  let mut parsed_orders: Signal<Vec<FileUploadOrderType>> = use_signal(|| vec![]);

  let read_files = move |file_engine: Arc<dyn FileEngine>| async move {
    let files = file_engine.files();
    for file_name in &files {
      if let Some(contents) = file_engine.read_file_to_string(&file_name).await {
        // files_uploaded.write().push(UploadedFile { name: file_name.clone(), contents });
        // info!("contents: {}", &contents);
        for order in contents.lines() {
          // info!("order: {}", order);
          match FileUploadOrder::parse(order) {
            Ok(valid_order) => {
              parsed_orders.write().push(valid_order.order);
            }
            Err(e) => {
              warn!("Parse error: {:?}", e);
            }
          }
        }

      }
    }
  };

  let upload_files = move |evt: FormEvent| async move {  
    if let Some(file_engine) = evt.files() {
      info!("file engine .files(): {:?}", file_engine.files());
      read_files(file_engine).await;
    }
    info!("event values: {:?}", evt.value());
  };

  let handle_click = move |evt| {
    let z = evt.target();
  };

  use_effect(move || {
    // let current_parsed_orders = parsed_orders.read();
    for o in parsed_orders.read().iter() {
      info!("order: {:?}", o);
    }
  });

  rsx! {
    div {
      class: "card",
      h1 {
        class: "card-title center",
        "Orderbook Analysis Dashboard"
      },
      Tabs { mode },
      if mode() == Mode::Simulation {
        div {
          class: "simulation-content",
          p {
            class: "center",
            style: "color: var(--text-muted)",
            "Configure simulation settings"
          },
          SettingsPanel { name: "Simulation Settings", form_data }
        }
      } else { 
        /*Ver 2*/
        button { 
          onclick: move |_| parsed_orders.write().clear(),
          "Clear files"
        }
        // document::eval(r#"
        //             const handleClick = event => {
        //               const { target = {} } = event || {};
        //               target.value = "";
        //             };
        //         "#)
        form {
          div {
            label {
              class: "upload-zone",
              for: "file-upload",
             "Choose order file to upload (TXT, CSV)",
              input {
                id: "file-upload",
                // class: "hidden",
                r#type: "file",
                accept: ".txt,.csv",
                oninput: upload_files,
                onclick: handle_click
              }
            }
          },
          div { 
            class: "order-preview",
            if parsed_orders.read().len() == 0 {
              p {"No files currently selected for upload"}
            } else {
              PreviewTable { orders: parsed_orders() }
            }
          }
          div { 
            button { "Submit" }
          }
        }
      },
    }
  }
}

#[component]
fn Tabs(mut mode: Signal<Mode>) -> Element {
  
  rsx! {
    div {
      class: "tabs",
      button {
        class: match mode() {
          Mode::Simulation => "tab active",
          _ => "tab"
        },
        onclick: move|_evt| mode.set(Mode::Simulation),
        "Simulation Mode"
      },
      button {
        class: match mode() {
          Mode::Upload => "tab active",
          _ => "tab"
        },
        onclick: move|_evt| mode.set(Mode::Upload),
        "Upload Mode"
      },
    }
  }
}

const GEAR_ICON: &str = r#"
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-settings"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>
"#;

#[component]
fn SettingsPanel(name: String, mut form_data: Signal<HashMap<String, FormValue>>) -> Element {
  rsx!{
    div {
      class: "center mt-4",
      button {
        class: "button settings-button",
        onclick: move |_evt| {
          document::eval(r#"
              const panel = document.getElementById('simulation-settings-panel');
              panel.classList.toggle('hidden');
              "#);
        },
        div {
          dangerous_inner_html: GEAR_ICON,
          class: "gear-icon"
        }
        {name}
      }
    },
    div {
      id: "simulation-settings-panel",
      class: "settings-panel hidden mt-4",
      Dialog { form_data }
    },
    div {
      id: "toast",
      "Settings saved successfully"
    }   
  }
}

#[component]
fn PreviewTable(orders: Vec<FileUploadOrderType>) -> Element {

  let len: usize = orders.len();
  let max: usize = 10;
  // we add 2 middle rows for ellipsis
  let mut preview_rows: Vec<PreviewRow> = Vec::with_capacity((2*max)+2);

  if len <= 2*max {
    for (idx, order) in orders.iter().enumerate() {
      preview_rows.push(get_preview_row(idx, order));
    }
  } else {
    // top 10 rows
    for (idx, order) in (0..).zip(orders.iter().take(max)) {
      preview_rows.push(get_preview_row(idx, order));
    }
    // 2 ellipsis rows
    let ellipsis_row_id1 = format!("preview-row-{}", max);
    let ellipsis_row_id2 = format!("preview-row-{}", max+1);
    preview_rows.push(PreviewRow { row_id: ellipsis_row_id1, ordertype: "⋮".to_string(), order_id: "⋮".to_string(), side: "⋮".to_string(), shares: "⋮".to_string(), price: "⋮".to_string() });
    preview_rows.push(PreviewRow { row_id: ellipsis_row_id2, ordertype: "⋮".to_string(), order_id: "⋮".to_string(), side: "⋮".to_string(), shares: "⋮".to_string(), price: "⋮".to_string() });
    // last 10 rows
    for (idx, order) in (max+2..).zip(orders.iter().skip(len-max)) { 
      preview_rows.push(get_preview_row(idx, order));
    }
  }
  
  rsx! {
    table {
      class: "upload-preview",
      tbody {
        tr {
          th { scope:"col", "OrderType" },
          th { scope:"col", "ID" },
          th { scope:"col", "Side" },
          th { scope:"col", "Shares" },
          th { scope:"col", "Price" },
        }
        for row in preview_rows {
          tr {
            key: "{row.row_id}",
            td {"{row.ordertype}"},
            td {"{row.order_id}"},
            td {"{row.side}"},
            td {"{row.shares}"},
            td {"{row.price}"},
          }
        }
      }
    }
  }
}


fn get_preview_row(idx:usize, order: &FileUploadOrderType) -> PreviewRow {
  let row_id = format!("preview-row-{idx}");
  match order {
    FileUploadOrderType::Add { id, side, shares, price } => PreviewRow { row_id, ordertype: "ADD".to_string(), order_id: id.to_string(), side: side.to_string() , shares: shares.to_string(), price: price.to_string() 
    },
    FileUploadOrderType::Modify { id, shares, price } => PreviewRow { row_id, ordertype: "MODIFY".to_string(), order_id: id.to_string(), side: "-".to_string() , shares: shares.to_string(), price: price.to_string() },
    FileUploadOrderType::Cancel { id } => PreviewRow { row_id, ordertype: "CANCEL".to_string(), order_id: id.to_string(), side: "-".to_string() , shares: "-".to_string(), price: "-".to_string() }
    
  }
}