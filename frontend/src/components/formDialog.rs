#![allow(non_snake_case)]

use std::collections::HashMap;
use dioxus::prelude::*;

#[component]
pub fn Dialog (
  mut form_data: Signal<HashMap<String, FormValue>>,
  mut is_valid_sim_settings: Signal<bool>
) -> Element {

  let mut add_prob: Signal<f32> = use_signal(||0.0);
  let mut modify_prob: Signal<f32> = use_signal(||0.6);
  let mut cancel_prob: Signal<f32> = use_signal(||0.4);
  let mut prob_sums: Signal<f32> = use_signal(||1.0);
  
  use_effect(move || {
    prob_sums.set(add_prob() + modify_prob() + cancel_prob());

    let current_sum = prob_sums();

    if (current_sum - 1.0).abs() < f32::EPSILON {
      //info!("sum of probs is valid: {:?}", current_sum);
      is_valid_sim_settings.set(true);
    } else {
      //warn!("sum of probs must be 1. current sum: {:?}", current_sum);
      is_valid_sim_settings.set(false);
    }
  });

  rsx! {
    form {
      id: "simulation-settings",
      onsubmit: move|evt| {
          //info!("form submitted with {:?}", evt.values() );
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
        input { class: "form-input", name: "orders", type: "number", min: "50000", max: "1500000", value: "50000" }
      },
      div {
        class: "form-group",
        label { class: "form-label", "Mean Price" },
        input { class: "form-input", name: "mean_price", r#type: "number", min: "100", max: "500", value: "250", step: "0.5"}
      },
      div {
        class: "form-group",
        label { class: "form-label", "Std Dev Price" },
        input { class: "form-input", name: "sd_price", r#type: "number", min: "5", max: "50", step: "0.5", value: "20"}
      },
      div {
        class: "form-group",
        label { class: "form-label", "Probabilites" }
        div {
          class: "prob-inputs",
            div {
            class: "prob-group",
            label { class: "prob-label", "ADD:"}
            input { 
              class: "prob-input",
              name: "add_prob",
              r#type: "number",
              min: "0",
              max: "1",
              step: "0.1",
              value: "0.0",
              oninput: move |evt| {
                if let Ok(value) = evt.value().parse::<f32>() {
                  add_prob.set(value);
                }
              }
            }
          }
          div {
            class: "prob-group",
            label { class: "prob-label", "MODIFY:"}
            input {
              class: "prob-input",
              name: "modify_prob",
              r#type: "number",
              min: "0",
              max: "1",
              step: "0.1",
              value: "0.6",
              oninput: move |evt| {
                if let Ok(value) = evt.value().parse::<f32>() {
                  modify_prob.set(value);
                }
              }
            }
          }
          div {
            class: "prob-group",
            label { class: "prob-label", "CANCEL:"}
            input {
              class: "prob-input",
              name: "cancel_prob",
              r#type: "number",
              min: "0",
              max: "1",
              step: "0.1",
              value: "0.4",
              oninput: move |evt| {
                if let Ok(value) = evt.value().parse::<f32>() {
                  cancel_prob.set(value);
                }
              }
            }
          }
          div {
            class: "sum-display",
            span { "Sum: " }
            span { id: "probs-sum-value", "{prob_sums():.1}" }
            if is_valid_sim_settings() {
              span { id: "probs-sum-valid", class: "form-valid-msg", "✓" }
            } else {
              span { id: "probs-sum-error", class: "form-error-msg", "Must equal 1.0" }
            }
          }
        }
      }
      div {
        class: "form-actions",
        if is_valid_sim_settings() {
          button { type: "submit" , class: "button button-primary", "Apply Settings"}
        },
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