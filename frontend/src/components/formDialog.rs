#![allow(non_snake_case)]

use std::collections::HashMap;
use dioxus::{logger::tracing::info, prelude::*};


#[component]
pub fn Dialog (mut form_data: Signal<HashMap<String, FormValue>>) -> Element {

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
                    input { name: "orders", type: "number", min: "5", max: "100000" }
                },
                p {
                    label { "Time" },
                    input { name: "time", type: "number" , min: "10", max: "3000"}
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
                  input { name: "mean_price", r#type: "number", min: "100", max: "500", step: "0.05"}
                },
                p {  
                  label { "Price Variation (Std Dev)" },
                  input { name: "sd_price", r#type: "number", min: "10", max: "100", step: "0.01"}
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