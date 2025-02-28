#![allow(non_snake_case)]

use std::collections::HashMap;
use dioxus::{logger::tracing::info, prelude::*};

/*TODO: Remove this
#[component]
pub fn DialogDeprecated (mut form_data: Signal<HashMap<String, FormValue>>) -> Element {

    let close_dialog = move || {
        document::eval(r#"
        const dialog = document.getElementById('favDialog');
        dialog.close();
        "#);
    };

    rsx! {
        dialog {
            id: "favDialog",
            form {
                id: "settingsForm",
                onsubmit: move|evt| {
                    close_dialog();
                    // info!("form submitted with {:?}", evt.values() );
                    let d = &mut form_data.write();
                    d.extend(evt.values());
                },
                p {
                    label { "Orders:" },
                    input { name: "orders", type: "number", min: "50000", max: "2500000" }
                },
                p {
                    label { "Time" },
                    input { name: "time", type: "number" , min: "3", max: "3000"}
                },
                p {
                    label { "Time units" },
                    select {
                        name: "units",
                        option { value: "ms", "ms" }
                        option { value: "μs", "μs" }
                    }
                },
                p {  
                  label { "Mean Price" },
                  input { name: "mean_price", r#type: "number", min: "100", max: "500", step: "0.5"}
                },
                p {  
                  label { "Price Variation (Std Dev)" },
                  input { name: "sd_price", r#type: "number", min: "5", max: "50", step: "0.05"}
                },
                p { 
                  fieldset {  
                    legend { "Select what price levels to show" },
                    div {  
                      input { name: "price_lvl", id: "by_best", r#type: "radio", value: "true" },
                      label { r#for: "by_best" , "Best bids & asks" }
                    },
                    div {  
                      input { name: "price_lvl", id: "by_root", r#type: "radio", value: "false" },
                      label { r#for: "by_root" , "By tree root" }
                    }
                  }
                },
                button { type: "submit" , "Save"},
                button { type: "button" , 
                onclick: move |_| {
                    document::eval(r#"
                    const dialog = document.getElementById('favDialog');
                    dialog.close();
                    "#);
                }, 
               
                "Cancel"}
            }
        }
    }
}
*/

#[component]
pub fn Dialog (mut form_data: Signal<HashMap<String, FormValue>>) -> Element {
  
  /*
  var x = document.getElementById("settings-success-toast");
              x.className = "show";
              setTimeout(function(){ x.className = x.className.replace("show", "");}, 2000);
              "#);
   */

  rsx! {
    form {
      id: "simulation-settings",
      onsubmit: move|evt| {
          info!("form submitted with {:?}", evt.values() );
          let d = &mut form_data.write();
          d.extend(evt.values());
          document::eval(r#"
              const panel = document.getElementById('simulation-settings-panel');
              panel.classList.add('hidden');
              const simStartBtn = document.getElementById('sim-start-btn');
              simStartBtn.classList.remove('hidden');
              "#);
          document::eval(r#"
              var x = document.getElementById("settings-toast");
              x.classList.add("show");
              setTimeout(function(){{x.classList.remove("show");}}, 2000);
              "#);
      },
      div {
        class: "form-group",
        label { class: "form-label", "Orders" },
        input { class: "form-input", name: "orders", type: "number", min: "50000", max: "2500000" }
      },
      div {
        class: "form-group",
        label { class: "form-label", "Mean Price" },
        input { class: "form-input", name: "mean_price", r#type: "number", min: "100", max: "500", step: "0.5"}
      },
      div {
        class: "form-group",
        label { class: "form-label", "Std Dev Price" },
        input { class: "form-input", name: "sd_price", r#type: "number", min: "5", max: "50", step: "0.5"}
      },
      div {
        class: "form-actions",
        button { type: "submit" , class: "button button-primary", "Apply Settings"},
        button {
          type: "button",
          class: "button",
          onclick: move|_evt| {
            document::eval(r#"
              const panel = document.getElementById('simulation-settings-panel');
              panel.classList.add('hidden');
              const simStartBtn = document.getElementById('sim-start-btn');
              simStartBtn.classList.remove('hidden');
              "#);
          },
          "Cancel"
        },

      }
    }
  }
}