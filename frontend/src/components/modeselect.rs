#![allow(non_snake_case)]

use std::collections::HashMap;
use std::time::Duration;
use dioxus::html::{FileEngine, HasFileData};
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use web_sys::{HtmlInputElement, wasm_bindgen::JsCast};
use crate::components::formDialog::Dialog;
use crate::components::toast::{ErrorToast, SuccessToast};
use crate::pages::simulator::{Mode, HEALTH_CHECK_URL, LARGE_UPLOAD_URL, SMALL_UPLOAD_URL};
use crate::utils::auth::AuthSignature;
use crate::utils::file_handler::{format_duration, FileUploadOrder, FileUploadOrderType, FinalStats, PreviewRow, UnifiedUploader};

static SMALL_FILE: Asset = asset!("assets/sample_small_file.txt");
static LARGE_FILE: Asset = asset!("assets/sample_large_file.txt");

#[component]
pub fn ModeSelector(mut mode: Signal<Mode>, mut form_data: Signal<HashMap<String, FormValue>>, mut is_valid_sim_settings: Signal<bool>) -> Element {
  
  let mut parsed_orders: Signal<Vec<FileUploadOrderType>> = use_signal(|| vec![]);
  let mut large_file_contents: Signal<Vec<u8>> = use_signal(||vec![]);
  let mut total_raw_orders: Signal<i32> = use_signal(|| 0);
  let mut selected_file: Signal<Option<String>> = use_signal(||None);
  let mut server_processing: Signal<Option<String>> = use_signal(||None);
  let mut invalid_file: Signal<bool> = use_signal(||false);
  let mut is_large_file: Signal<bool> = use_signal(||false);
  let mut ob_results: Signal<Option<HashMap<String, FinalStats>>> = use_signal(||None);
  let mut parse_results: Signal<Option<(Duration, i32, i32)>> = use_signal(||None);

  // set max size to 5MB for which we show preview and do UI side order parsing 
  const MAX_PREVIEWABLE_FILESIZE: u64 = 1024 * 1024 * 5;
  let uploader = use_signal(||UnifiedUploader::new(reqwest::Client::new(), SMALL_UPLOAD_URL, LARGE_UPLOAD_URL, HEALTH_CHECK_URL).with_compression(true));

  let read_files = move |file_engine: std::sync::Arc<dyn FileEngine>| async move {
    let current_file_names = file_engine.files();
    if current_file_names.len() > 0 {
      
      let file_name = &current_file_names[0];
      selected_file.set(Some(file_name.clone()));

      if !file_name.to_lowercase().ends_with(".txt") {
        invalid_file.set(true);
        //error!("file name doesnt end with .txt!");
        return;
      }

      let file_size = file_engine.file_size(&file_name).await.expect("error getting uploaded file size!");

      // parse small files client side
      if file_size < MAX_PREVIEWABLE_FILESIZE {

        if let Some(contents) = file_engine.read_file_to_string(&file_name).await {
          for order in contents.lines() {
            *total_raw_orders.write() += 1;
            match FileUploadOrder::parse(order) {
              Ok(valid_order) => parsed_orders.write().push(valid_order.order),
              Err(_e) => {
                //warn!("parse error: {:?}", e);
                //*invalid_orders.write() += 1;
              }
            }
          }
        }
      } else {
        //warn!("large file uploaded of size: {:?} bytes. No preview will be shown", file_size);
        is_large_file.set(true);
        if let Some(file_bytes) = file_engine.read_file(&file_name).await {
          large_file_contents.set(file_bytes);
        }
      }
    }
  };

  let upload_files = move |evt: FormEvent| async move {
    if let Some(file_engine) = evt.files() {
      invalid_file.set(false);
      is_large_file.set(false);
      parsed_orders.write().clear();
      large_file_contents.write().clear();
      total_raw_orders.set(0);
      selected_file.set(None);
      ob_results.set(None);
      parse_results.set(None);
      read_files(file_engine).await;
      // clear file inputs to enable reupload of same file
      if let Some(web_evt) = evt.try_as_web_event() {
        if let Some(tar) = web_evt.target() {
          if let Ok(input_element) = tar.dyn_into::<HtmlInputElement>() {
            input_element.set_value("");
          }
        }
      }
    }
  };

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
          SettingsPanel { name: "Simulation Settings", form_data, is_valid_sim_settings }
        }
      } else { 
        form {
          class: "file-upload-form",
          enctype: "multipart/form-data",
          onsubmit: move|_evt| async move {
            let f_name = selected_file().expect("file name should exist here!");
            //info!("[on submit] file name: {}", &f_name); 
            server_processing.set(Some(f_name.clone()));
            selected_file.set(None);

            let upload_handler = uploader.read();
            let auth_signer = AuthSignature::new().await.expect("failed to init the auth signature!");
            // perform health check
            if let Err(_e) = upload_handler.check_health().await {
              //error!("health check error: {:?}", e);
              document::eval(r#"
              var x = document.getElementById("upload-server-down-toast");
              x.classList.add("show");
              setTimeout(function(){{x.classList.remove("show");}}, 2000);
              "#);
              server_processing.set(None);
              return;
            };

            if !is_large_file() {
              let current_orders = parsed_orders();
              parsed_orders.write().clear();

              if let Err(_e) = upload_handler.upload_small_file(current_orders, 10_000, auth_signer, ob_results).await {
                //error!("[Small upload error] {}", e.to_string());
                document::eval(r#"
                var x = document.getElementById("upload-server-down-toast");
                x.classList.add("show");
                setTimeout(function(){{x.classList.remove("show");}}, 2000);
                "#);
              }
            } else {
              let current_lf_bytes = large_file_contents();
              large_file_contents.write().clear();

              if let Err(_e) = upload_handler.upload_large_file(current_lf_bytes, &f_name, auth_signer, ob_results, parse_results).await {
                //error!("[Large upload error] {}", e.to_string());
                document::eval(r#"
                var x = document.getElementById("upload-server-down-toast");
                x.classList.add("show");
                setTimeout(function(){{x.classList.remove("show");}}, 2000);
                "#);
              };
            }
            server_processing.set(None);
          },
          div {
            class: "upload-container",
            h3 { "Upload Text Files" }
            p { class: "upload-subtitle", "Upload your .txt files for processing" }
            div {
              class: "upload-area",
              id: "dropzone",
              onmounted: move |_evt| {
                document::eval(r#"
                  var millis = 150;
                  setTimeout(function() {{
                     const dropZone = document.getElementById('dropzone');
                     if (!dropZone) {console.warn('no drop zone found!');}
                     dropZone.addEventListener('dragover', (e) => {
                        e.preventDefault();
                        dropZone.classList.add('dragover');
                     });
                      dropZone.addEventListener('drop', (e) => {
                        e.preventDefault();
                        dropZone.classList.remove('dragover');
                      });
                  }}, millis);
                "#);
              },

              ondrop: move |evt| async move {
                if let Some(file_engine) = evt.files() {
                  invalid_file.set(false);
                  is_large_file.set(false);
                  parsed_orders.write().clear();
                  large_file_contents.write().clear();
                  total_raw_orders.set(0);
                  selected_file.set(None);
                  ob_results.set(None);
                  parse_results.set(None);
                  read_files(file_engine).await;
                  // clear file inputs to enable reupload of same file
                  if let Some(web_evt) = evt.try_as_web_event() {
                    if let Some(tar) = web_evt.target() {
                      if let Ok(input_element) = tar.dyn_into::<HtmlInputElement>() {
                        input_element.set_value("");
                      }
                    }
                  }
                }
              },
              div { class: "upload-icon", "📄" }
              p {class: "upload-text", "Drag & drop your text files here"}
              p { "or" }
              button {
                type: "button",
                id: "browse-button",
                onclick: move |evt| {
                  evt.prevent_default();
                  document::eval(r#"
                    const fileInput = document.getElementById('file-upload');
                    fileInput.click();
                  "#);
                },
                ondragover: move|evt| evt.prevent_default(),
                ondrop: move|evt| evt.prevent_default(),
                "Click to browse your files"
              }
              input {
                r#type: "file",
                id: "file-upload",
                class: "file-input",
                accept: ".txt",
                onchange: upload_files
              }
            }
            if let Some(file_name) = selected_file() {
              div {
                class: "file-info",
                id: "fileInfo",
                div {
                  class: "file-name",
                  span { id: "fileName", "{file_name}" }
                  span {
                    id: "removeFile",
                    style: "cursor: pointer",
                    onclick: move |_evt| selected_file.set(None),
                    "✕"
                  }
                }
              }
              if !invalid_file() {
                if !is_large_file() {
                  if parsed_orders().len() > 0 {
                    div { 
                      button { 
                        id : "file-upload-btn",
                        r#type: "submit",
                        "Submit"
                      }
                    }
                  } else {
                    div { class: "upload-err-msg parsedorders", "Found 0 valid orders from ""{total_raw_orders()}"" lines! Refer file format here." }
                  }
                } else {
                  div { 
                    button { 
                      id : "file-upload-btn",
                      r#type: "submit",
                      "Submit"
                    }
                  }
                  span {"No preview available for files > 5MB"}
                }
              } else {
                div { class: "upload-err-msg fileformat", "Only .txt files are allowed!" }
              }
            }
          }
          if selected_file().is_some() && !is_large_file() && parsed_orders().len() > 0 {
            PreviewTable { orders: parsed_orders() }
          }
          if let Some(f_name) = server_processing() { ProgressBar { f_name } }
        }

        if server_processing().is_none() && selected_file().is_none() && ob_results().is_none() {
          SampleFile {}
        }
        if ob_results().is_some() {
          ResultsTable { orders: ob_results().expect("results should exist here!") }
        }
        if parse_results().is_some() {
          ParseTable { parse_results: parse_results().expect("parse results should exist here!") }
        }
        ErrorToast { id: "upload-server-down-toast", content: "SERVER IS DOWN! Try again later." }
        ErrorToast { id: "upload-server-rl-toast", content: "Max order limit reached! Please try again in some time." }
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

#[component]
fn SettingsPanel(name: String, mut form_data: Signal<HashMap<String, FormValue>>, mut is_valid_sim_settings: Signal<bool>) -> Element {

  let mut order_str = use_signal(||"50000".to_string());
  let mut mean_str = use_signal(||"250".to_string());
  let mut sd_str = use_signal(||"20".to_string());
  let mut add_prob_str = use_signal(||"0.0".to_string());
  let mut modify_prob_str = use_signal(||"0.6".to_string());
  let mut cancel_probs_str = use_signal(||"0.4".to_string());

  use_effect(move|| {
    let current_form_data = form_data();

    if let Some(order) = current_form_data.get("orders") {
      order_str.set(order.as_value());
    };
    if let Some(mean) = current_form_data.get("mean_price") {
      mean_str.set(mean.as_value());
    };
    if let Some(sd) = current_form_data.get("sd_price") {
      sd_str.set(sd.as_value());
    };
    if let Some(add_prob) = current_form_data.get("add_prob") {
      add_prob_str.set(add_prob.as_value());
    }
    if let Some(modify_prob) = current_form_data.get("modify_prob") {
      modify_prob_str.set(modify_prob.as_value());
    }
    if let Some(cancel_prob) = current_form_data.get("cancel_prob") {
      cancel_probs_str.set(cancel_prob.as_value());
    }
  });
  
  rsx!{
    div {
      class: "center mt-4",
      button {
        class: "button button-sim-settings",
        onclick: move |_evt| { 
          document::eval(r#"
              const panel = document.getElementById('simulation-settings-panel');
              panel.classList.toggle('hidden');
              const simStartBtn = document.getElementById('sim-start-btn');
              simStartBtn.classList.toggle('hidden');
              "#);
        },
        div {
          class: "gear-icon",
          svg {
            xmlns: "http://www.w3.org/2000/svg",
            width: "24",
            height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentcolor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path {
              d: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
            },
            circle { cx: "12", cy: "12", r: "3" }
          }
        }
        {name}
      }
    },
    div {
      class: "param-display",
      div {
        class: "param-display-header",
        span { class: "param-display-title", "Current Simulation Parameters" }
      }
      div { 
        class: "param-badges",
        div {
          class: "param-badge",
          span { class: "param-badge-label", "Orders:" }
          span { class: "param-badge-value", {order_str} }
        }
        div {
          class: "param-badge",
          span { class: "param-badge-label", "Mean Price:" }
          span { class: "param-badge-value", {mean_str} }
        }
        div {
          class: "param-badge",
          span { class: "param-badge-label", "Variation:" }
          span { class: "param-badge-value", {sd_str} }
        }
        div {
          class: "param-badge",
          span { class: "param-badge-label", "ADD:" }
          span { class: "param-badge-value", {add_prob_str} }
        }
        div {
          class: "param-badge",
          span { class: "param-badge-label", "MODIFY:" }
          span { class: "param-badge-value", {modify_prob_str} }
        }
        div {
          class: "param-badge",
          span { class: "param-badge-label", "CANCEL:" }
          span { class: "param-badge-value", {cancel_probs_str} }
        }
      }
    }
    div {
      id: "simulation-settings-panel",
      class: "settings-panel hidden mt-4",
      Dialog { form_data, is_valid_sim_settings }
    }, 
    SuccessToast { id: "settings-toast", content: "Settings saved successfully" }
  }
}

#[component]
fn PreviewTable(orders: Vec<FileUploadOrderType>) -> Element {

  let len: usize = orders.len();
  let max: usize = 5;
  // add 2 middle rows (...) for ellipsis
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
    // ... rows
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
      class: "upload-preview-table",
      caption { "File preview" }
      thead {
        tr {
          th { scope:"col", "Order Type" },
          th { scope:"col", "ID" },
          th { scope:"col", "Side" },
          th { scope:"col", "Shares" },
          th { scope:"col", "Price" },
        }
      }
      tbody {
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

#[component]
fn ResultsTable(orders: HashMap<String, FinalStats>) -> Element {

  struct FormattedStats { total_time: String, avl_rebalances: i64, executed_orders_cnt: i64, nos: i64 }
  
  let cumlative_stats = orders.values().fold(FinalStats {
    total_time: Duration::new(0, 0),
    avl_rebalances: 0,
    executed_orders_cnt: 0,
    nos: 0}, |acc, stats| acc.add(stats));

  let cumlative_formatted_stats = FormattedStats { 
    total_time: format_duration(cumlative_stats.total_time), avl_rebalances: cumlative_stats.avl_rebalances,
    executed_orders_cnt: cumlative_stats.executed_orders_cnt,
    nos: cumlative_stats.nos
  };

  let mut formatted_orders: HashMap<String, FormattedStats> = HashMap::new();

  for (key, stats) in &orders {
    let formatted_time = format_duration(stats.total_time);
    formatted_orders.insert(
      key.to_string(), FormattedStats { 
        total_time: formatted_time,
        avl_rebalances: stats.avl_rebalances,
        executed_orders_cnt: stats.executed_orders_cnt,
        nos: stats.nos 
      }
    );
  }

  rsx! {
    table { 
      class: "upload-results-table ob",
      caption { "Orderbook results" }
      thead { 
        tr { 
          th {
            scope: "col",
            rowspan: "2",
            "Order Type"
          },
          th {
            scope: "col",
            rowspan: "2",
            "Order Count"
          },
          th {
            scope: "col",
            colspan: "3",
            "Order Book Stats"
          },
        },
        tr { 
          th { 
            scope: "col",
            "AVL Rebalances"
          },
          th { 
            scope: "col",
            "Trades Executed"
          },
          th { 
            scope: "col",
            "Execution Time"
          }
        }
       }
       tbody { 
        for (key, stats) in formatted_orders {
          tr { 
            key: "{key}-result",
            th { scope: "row", "{key}" },
            td { "{stats.nos}" },
            td { "{stats.avl_rebalances}" },
            td { "{stats.executed_orders_cnt}" },
            td { "{stats.total_time}" },
          }
        }
      }
      tfoot { 
        tr {
          th { scope: "row", colspan: "1", "Total" },
          td { "{cumlative_formatted_stats.nos}" },
          td { "{cumlative_formatted_stats.avl_rebalances}" },
          td { "{cumlative_formatted_stats.executed_orders_cnt}" },
          td { "{cumlative_formatted_stats.total_time}" }
        }
      }
    }
  }
}

#[component]
fn ParseTable(parse_results: (Duration, i32, i32)) -> Element {
  
  let (t, lines, invalid_orders) = parse_results;
  let formatted_time = format_duration(t);
  let valid_orders = lines - invalid_orders;

  rsx! {
    table {
      class: "upload-results-table parse",
      caption { "File parsing results" }
      thead { 
        tr { 
          th {
            scope: "col",
            "Total parsed lines"
          },
          th {
            scope: "col",
            "Valid Orders"
          },
          th {
            scope: "col",
            "Invalid Orders"
          },
          th {
            scope: "col",
            "Time"
          }
        }
      }
      tbody { 
        tr {
          td { "{lines}" },
          td { "{valid_orders}" },
          td { "{invalid_orders}" },
          td { "{formatted_time}" }
        }
      }
    }
  }
}

#[component]
fn ProgressBar(f_name: String) -> Element {
  rsx! {
    div {
      class: "upload-file-proc-container",
      div {
        class: "upload-file-proc-label",
        "UPLOADING FILE"
      }
      div {
        class: "progress-container",
        div {
          class: "indeterminate-progress-bar"
        }
      }
      div {
        class: "upload-file-proc-status",
        span { "Processing..." }
      }
      div {
        class: "upload-file-details",
        div { 
          class: "upload-file-name",
          {f_name}
        }
      }
    }
  }
}

#[component]
fn SampleFile() -> Element {
  rsx! {
    div {
      class: "sample-file-container",
      div {
        class: "sample-file-section",
        div { class: "sample-file-header", "Sample Order Files"}
        p { class: "sample-file-desc", "Don't have a order file? Download one of the sample files to test the engine:"}
        div { 
          class: "sample-files",
          div {
            class: "sample-file-card",
            div {
              class: "sample-file-icon",
              svg {
                width: "20",
                height: "20",
                view_box: "0 0 24 24",
                fill: "none",
                xmlns: "http://www.w3.org/2000/svg",
                path { 
                  d: "M14 2H6C5.46957 2 4.96086 2.21071 4.58579 2.58579C4.21071 2.96086 4 3.46957 4 4V20C4 20.5304 4.21071 21.0391 4.58579 21.4142C4.96086 21.7893 5.46957 22 6 22H18C18.5304 22 19.0391 21.7893 19.4142 21.4142C19.7893 21.0391 20 20.5304 20 20V8L14 2Z",
                  stroke: "currentColor",
                  stroke_width: "2",
                  stroke_linecap: "round",
                  stroke_linejoin: "round"
                }
                path { 
                  d: "M14 2V8H20",
                  stroke: "currentColor",
                  stroke_width: "2",
                  stroke_linecap: "round",
                  stroke_linejoin: "round"
                }
              }
            }
            div {
              class: "sample-file-info",
              div { class: "sample-file-name", "Small Sample" }
              div { class: "sample-file-details", "1KB • 8 orders" }
            }
            a { 
              class: "sample-file-download-icon",
              href: SMALL_FILE,
              download: "small_file",
              svg {
                width: "24",
                height: "24",
                xmlns: "http://www.w3.org/2000/svg",
                view_box: "0 0 24 24",
                fill: "none",
                path {
                  d: "M12 15L12 3M12 15L8 11M12 15L16 11M21 15V19C21 19.5304 20.7893 20.0391 20.4142 20.4142C20.0391 20.7893 19.5304 21 19 21H5C4.46957 21 3.96086 20.7893 3.58579 20.4142C3.21071 20.0391 3 19.5304 3 19V15",
                  stroke: "currentColor",
                  stroke_linecap: "round",
                  stroke_linejoin: "round"
                }
              }
            }
          }
          div {
            class: "sample-file-card",
            div {
              class: "sample-file-icon",
              svg {
                width: "20",
                height: "20",
                view_box: "0 0 24 24",
                fill: "none",
                xmlns: "http://www.w3.org/2000/svg",
                path { 
                  d: "M14 2H6C5.46957 2 4.96086 2.21071 4.58579 2.58579C4.21071 2.96086 4 3.46957 4 4V20C4 20.5304 4.21071 21.0391 4.58579 21.4142C4.96086 21.7893 5.46957 22 6 22H18C18.5304 22 19.0391 21.7893 19.4142 21.4142C19.7893 21.0391 20 20.5304 20 20V8L14 2Z",
                  stroke: "currentColor",
                  stroke_width: "2",
                  stroke_linecap: "round",
                  stroke_linejoin: "round"
                }
                path { 
                  d: "M14 2V8H20",
                  stroke: "currentColor",
                  stroke_width: "2",
                  stroke_linecap: "round",
                  stroke_linejoin: "round"
                }
              }
            }
            div {
              class: "sample-file-info",
              div { class: "sample-file-name", "Large Sample" }
              div { class: "sample-file-details", "34MB • 1.5million orders" }
            }
            a { 
              class: "sample-file-download-icon",
              href: LARGE_FILE,
              download: "large_file",
              svg {
                width: "24",
                height: "24",
                xmlns: "http://www.w3.org/2000/svg",
                view_box: "0 0 24 24",
                fill: "none",
                path {
                  d: "M12 15L12 3M12 15L8 11M12 15L16 11M21 15V19C21 19.5304 20.7893 20.0391 20.4142 20.4142C20.0391 20.7893 19.5304 21 19 21H5C4.46957 21 3.96086 20.7893 3.58579 20.4142C3.21071 20.0391 3 19.5304 3 19V15",
                  stroke: "currentColor",
                  stroke_linecap: "round",
                  stroke_linejoin: "round"
                }
              }
            }
          }
        }
      }
    }
  }
}