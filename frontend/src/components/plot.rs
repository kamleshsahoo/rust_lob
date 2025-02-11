#![allow(non_snake_case)]

use std::collections::HashMap;
use charming::{component::{Axis, Grid, Title}, element::{AxisLabel, AxisType, Color, ColorBy, LineStyle, NameLocation, SplitLine, TextStyle, Tooltip}, series::Bar, theme::Theme, Chart, WasmRenderer};
use dioxus::{logger::tracing::{info, warn}, prelude::*};
use crate::{utils::enginestats::bin_data, PlotPropsState};

static  CANVAS_ID_HIST: &str = "latency-hist";
static  CANVAS_ID_BAR: &str = "latency-bar";

#[component]
pub fn BarPlotCharming(latency_by_ordertype: ReadOnlySignal<HashMap<String, Vec<f64>>>) -> Element {
  //let renderer = use_signal(|| WasmRenderer::new(500, 281).theme(Theme::Halloween));
  let renderer = use_signal(|| WasmRenderer::new_opt(None, None).theme(Theme::Halloween));
  let latency_lim = use_context::<PlotPropsState>().latency_cutoff;
  //let y_max = 1.05 * (latency_lim() as f64);
  let y_max = latency_lim();

  use_effect(move || {
      
    let x_labels = vec!["ADD", "MODIFY", "CANCEL"];
    let mut avg_lat: Vec<i64> = vec![];
    
    for (idx, k) in x_labels.iter().enumerate() {
      if let Some(vec) = latency_by_ordertype().get(*k) {
        let avg = *vec.last().expect("failed to get avg as last elem");
        avg_lat.push(avg as i64);
      }
    }

    let chart = Chart::new()
    .title(
      Title::new()
      .text("Latency by Ordertype")
      .text_style(
        TextStyle::new()
        .color("rgba(255, 255, 255, 1)")
        // .padding_all(150)
        .font_family("monospace")
        .font_size(20)
      )
    )
    .background_color("rgba(41,52,65,1)")
    .color(vec![Color::Value("#fc97af".to_string()), Color::Value("#87f7cf".to_string()), Color::Value("#72ccff".to_string())])
    .tooltip(
      Tooltip::new()
      .formatter("{a}: {c}ns")
    )
    .grid(
      Grid::new()
      .left("10%")
      .contain_label(true)
    )
    .x_axis(
      Axis::new()
      .type_(AxisType::Category)
      .data(x_labels)
    )
    .y_axis(
      Axis::new()
      .type_(AxisType::Value)
      .max(y_max)
    )
    .series(
      Bar::new()
      .name("Avg latency")
      // .show_background(true).background_style(BackgroundStyle::new().color("rgba(180, 180, 180, 0.2)"))
      .color_by(ColorBy::Data)
      .data(avg_lat)
    );

    renderer.read_unchecked().render(CANVAS_ID_BAR, &chart).expect("failed to create charming bar plot!");
  
  });

  rsx! {
    div {
      id: CANVAS_ID_BAR,
      onmounted: move |_evt| {
        document::eval(
          r#"
          var millis = 350;
          setTimeout(function() {
              const element = document.getElementById('latency-bar');
              if (!element) {console.log('no element found');}
              var chart = echarts.getInstanceByDom(element);
              if (!chart) {console.log('no chart found');}
              window.addEventListener('resize', function() {
                  chart.resize();
              });
          }, millis)
          "#);
      }
    }
  }
}


#[component]
pub fn HistPlotCharming(latency: ReadOnlySignal<Vec<i64>>) -> Element {

  let renderer = use_signal(|| WasmRenderer::new_opt(None, None));
  let latency_lim = use_context::<PlotPropsState>().latency_cutoff;
  let freq_lim = use_context::<PlotPropsState>().frequency_cutoff;

  use_effect(move || {
    // info!("current latency lenght: {:?}", latency.len());
    let max_latency = latency_lim();
    let max_freq = freq_lim();
    let latency_bin_width: i64 = if max_latency < 20_000 { 250 } else { 500 };
    //let x_labels = (0..max_latency).step_by(latency_bin_width as usize).collect::<Vec<i64>>();
    
    let binned_data = bin_data(&latency(), latency_bin_width, max_latency);
    //info!("binned data: {:?}", &binned_data);
    // let tooltip = FormatterFunction::new_with_args(
    //   "params", 
    //   r#"
    //   var target = params[0];
    //   return target.seriesName + '<br/>' + yoy;
    // "#);
    // let z = js_sys::Function::new_with_args("params", 
    // r#"
    // var target = params[0];
    // return target.seriesName + '<br/>' + yoy;
    // "#);
    // let tooltip = Formatter::Function(FormatterFunction { value: z });
    
    //Formatter::Function( web_sys::js_sys::Function::new_with_args("params", r#"
    //var target = params[0];
    //return target.seriesName + '<br/>' + yoy; 
    //"#));
    
    let chart = Chart::new()
    //see if Tooltip,  legend, grid required
    .title(
      Title::new()
      .text("Order Latency distribution")
      // .subtext("Latency (in nanoseconds)")
      .text_style(
        TextStyle::new()
        .color("rgba(255, 255, 255, 1)")
        // .padding_all(150)
        .font_family("monospace")
        .font_size(20)
      )
    )
    .background_color("rgba(41,52,65,1)") //rgba(91,92,110,1) rgba(0,0,0,0.3) rgba(64,64,64,0.5)
    .color(vec![Color::Value("#72ccff".to_string())]) //rgba(255, 113, 94, 1)
    .tooltip(
      Tooltip::new()
      // .formatter(tooltip)
      // .trigger(Trigger::Axis)
      // .axis_pointer(
      //   AxisPointer::new().type_(AxisPointerType::Shadow)
      // )
      
      // .formatter( 
        
      //   Formatter::Function(r#"
      // function (params) {
      // var target = params[0];
      // return target.seriesName + '<br/>' + yoy; 
      // }
      // "#.into())
      // )
        // .formatter(r#"a:{a} b:{b} c:{c}"#)
      // .formatter( Formatter::String("{a0}: {c1}".into()))
      //.formatter(r#"{b0}<br />{a0}: {c1}"#)
      //{a} for series name, {b} for category name, {c} for data value, {d} for none;
    )
    .grid(
      Grid::new()
      .show(false)
      // .left("5%")
      // .right("10%")
      // .bottom("5%")
      // .contain_label(true)
    )
    
    .x_axis(
      Axis::new()
      .name("Latency (in nanoseconds)")
      .name_location(NameLocation::Middle)
      .name_gap(30.0)
      .name_text_style(
        TextStyle::new()
        .color("#ffffff")
        .font_family("Courier New")
        .font_size(14)
      )
      .scale(true)
      .split_line(
        SplitLine::new()
        .line_style(
          LineStyle::new()
          .color("#737373")
        )
      )
      .axis_label(
        AxisLabel::new()
        .color("#aaaaaa")
      )
    )
    .y_axis(
      Axis::new()
      .name("No. of orders")
      .name_location(NameLocation::Middle)
      .name_gap(32.0)
      .name_text_style(
        TextStyle::new()
        .color("#ffffff")
        .font_family("Courier New")
        .font_size(14)
      )
      .type_(AxisType::Value)
      .split_line(
        SplitLine::new()
        .line_style(
          LineStyle::new()
          .color("#737373")
        )
      )
      .axis_label(
        AxisLabel::new()
        .color("#aaaaaa")
      )
      .max(max_freq)
    )
    .series(
      Bar::new()
      .name("Count:")
      .data(binned_data)
      // .emphasis(
      //   Emphasis::new()
      //   .item_style(
      //     ItemStyle::new()
      //     .shadow_blur(10)
      //     .shadow_color("rgba(105, 180, 224, 0.3)")
      //   ))
      // .show_background(true).background_style(BackgroundStyle::new().color("rgba(180, 180, 180, 0.2)"))
      // .data(avg_lat)
    );

    renderer.read_unchecked().render(CANVAS_ID_HIST, &chart).expect("failed to create charming hist plot!");

  });

  rsx! {
    div {
      id: CANVAS_ID_HIST,
      onmounted: move |_evt| {
        document::eval(
          r#"
          var millis = 150;
          setTimeout(function() {
              const element = document.getElementById('latency-hist');
              if (!element) {console.log('no element found');}
              var chart = echarts.getInstanceByDom(element);
              if (!chart) {console.log('no chart found');}
              window.addEventListener('resize', function() {
              chart.resize();
              });
          }, millis)
          "#);
      }
    }
  }
}