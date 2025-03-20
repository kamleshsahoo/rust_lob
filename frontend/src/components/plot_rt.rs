#![allow(non_snake_case)]

use std::{collections::{BTreeMap, HashMap}, time::Duration};
use charming::{component::{Axis, Axis3D, Grid, Grid3D, Title}, element::{AxisLabel, AxisType, Color, ColorBy, FormatterFunction, LineStyle, NameLocation, SplitLine, TextStyle, Tooltip}, series::{Bar, Bar3d}, Chart, WasmRenderer};
use dioxus::prelude::*;
use js_sys::wasm_bindgen::JsValue;
use crate::utils::enginestats::{bar3d_data, bin_data};
use crate::pages::simulator::PlotPropsState;

static  CANVAS_ID_HIST: &str = "rt-latency-hist";
static  CANVAS_ID_BAR: &str = "rt-latency-bar";
static  CANVAS_ID_BAR3D: &str = "rt-latency-3d";

#[component]
pub fn HistPlotCharming(latency: ReadOnlySignal<Vec<i64>>) -> Element {

  let renderer = use_signal(|| WasmRenderer::new_opt(None, Some(350)));
  let latency_lim = use_context::<PlotPropsState>().latency_cutoff;
  let freq_lim = use_context::<PlotPropsState>().frequency_cutoff;

  use_effect(move || {
    // info!("current latency lenght: {:?}", latency.len());
    let max_latency = latency_lim();
    let max_freq = freq_lim();
    let latency_bin_width: i64 = if max_latency < 20_000 { 250 } else { 500 };
    
    let binned_data = bin_data(&latency(), latency_bin_width, max_latency);
    //info!("binned data: {:?}", &binned_data);
    
    let chart = Chart::new()
    .title(
      Title::new()
      .text("Order latency distribution")
      .text_style(
        TextStyle::new()
        .color("rgba(255, 255, 255, 1)")
        .font_family("Arial")
        .font_size(18)
      )
      // top, right, bottom, left
      .padding((12, 0, 5, 20))
    )
    .background_color("rgba(41,52,65,1)")
    .color(vec![Color::Value("#72ccff".to_string())]) //rgba(255, 113, 94, 1)
    .tooltip(
      Tooltip::new()
      // params.value = [MeanOfV0V1, VCount, V0, V1, DisplayableName]
      .formatter(FormatterFunction::new_with_args(
        "params",
        r#"
        var vals = params.value;
        return 'Latency : ' + vals[4] + '<br/>' + params.seriesName + ' : ' + vals[1];
        "# 
      ))
    )
    .grid(
      Grid::new()
      .show(false)
      .contain_label(true)
      .left("10%")
      .bottom("14%")
      .right("8%")
    )
    .x_axis(
      Axis::new()
      .name("Latency (in nanoseconds)")
      .name_location(NameLocation::Middle)
      .name_gap(30.0)
      .name_text_style(
        TextStyle::new()
        .color("#ffffff")
        .font_family("Consolas")
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
        .color("#cccccc")
      )
    )
    .y_axis(
      Axis::new()
      .name("No. of orders")
      .name_location(NameLocation::Middle)
      .name_gap(50.0)
      .name_text_style(
        TextStyle::new()
        .color("#ffffff")
        .font_family("Consolas")
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
        .color("#cccccc")
      )
      .max(max_freq)
    )
    .series(
      Bar::new()
      .name("Orders")
      .data(binned_data)
    );

    let _z = renderer.read_unchecked().render(CANVAS_ID_HIST, &chart).expect("failed to create charming hist plot!");
  });

  rsx! {
    div {
      id: CANVAS_ID_HIST,
      onmounted: move |_evt| {
        document::eval(
          r#"
            var millis = 150;
            setTimeout(function() {                
                const element_hist = document.getElementById('rt-latency-hist');
                if (!element_hist) {console.log('no element_hist found');}
                var chart_hist = echarts.getInstanceByDom(element_hist);
                if (!chart_hist) {console.log('no latency hist chart found');}
                const resizeObserver = new ResizeObserver(entries => {
                  for (const entry of entries) {
                    if (entry.target === element_hist) {
                      chart_hist.resize();
                    } 
                  }
                });
                resizeObserver.observe(element_hist);
                chart_hist.setOption({
                animation: false,
                });
            }, millis);
          "#
        );
      }
    }
  }
}

#[component]
pub fn BarPlotCharming(latency_by_ordertype: ReadOnlySignal<HashMap<String, Vec<f64>>>) -> Element {
  
  let renderer = use_signal(|| WasmRenderer::new_opt(None, Some(300)));
  let y_max = (use_context::<PlotPropsState>().avg_latency_cutoff)();

  use_effect(move || {
    let x_labels = vec!["ADD", "MODIFY", "CANCEL"];
    let mut avg_lat: Vec<i64> = vec![];

    //info!("avg lat for bar plot: {:?}", latency_by_ordertype());
    for (_idx, k) in x_labels.iter().enumerate() {
      if let Some(vec) = latency_by_ordertype().get(*k) {
        // NOTE: last element is the mean. See enginestats.rs 
        let avg = *vec.last().expect("failed to get last element as mean latency by ordertype!");
        avg_lat.push(avg as i64);
      } else {
        avg_lat.push(0);
      }
    }

    let chart = Chart::new()
    .title(
      Title::new()
      .text("Avg. latency by ordertype")
      .text_style(
        TextStyle::new()
        .color("rgba(255, 255, 255, 1)")
        .font_family("Arial")
        .font_size(18)
      )
      // top, right, bottom, left
      .padding((12, 0, 5, 20))
    )
    .background_color("rgba(41,52,65,1)")
    .color(vec![Color::Value("#fc97af".to_string()), Color::Value("#87f7cf".to_string()), Color::Value("#72ccff".to_string())])
    .tooltip(
      Tooltip::new()
      .formatter("{a}: {c} ns")
    )
    .grid(
      Grid::new()
      .left("12%")
      .top("23%")
      .bottom("14%")
      .right("6%")
      .contain_label(true)
    )
    .x_axis(
      Axis::new()
      .type_(AxisType::Category)
      .name("Order type")
      .name_location(NameLocation::Middle)
      .name_gap(28.0)
      .name_text_style(
        TextStyle::new()
        .color("#ffffff")
        .font_family("Consolas")
        .font_size(14)
      )
      .data(x_labels)
      .axis_label(
        AxisLabel::new()
        .color("#cccccc")
      )
    )
    .y_axis(
      Axis::new()
      .type_(AxisType::Value)
      .name("Latency (ns)")
      .name_location(NameLocation::Middle)
      .name_gap(60.0)
      .name_text_style(
        TextStyle::new()
        .color("#ffffff")
        .font_family("Consolas")
        .font_size(14)
      )
      .split_line(
        SplitLine::new()
        .line_style(
          LineStyle::new()
          .color("#737373")
        )
      )
      .axis_label(
        AxisLabel::new()
        .color("#cccccc")
      )
      .max(y_max)
    )
    .series(
      Bar::new()
      .name("Avg latency")
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
            var millis = 150;
            setTimeout(function() {
                const element_bar = document.getElementById('rt-latency-bar');
                if (!element_bar) {console.log('no element_bar found');}
                var chart_bar = echarts.getInstanceByDom(element_bar);
                if (!chart_bar) {console.log('no latency bar chart found');}
                const resizeObserver = new ResizeObserver(entries => {
                  for (const entry of entries) {
                    if (entry.target === element_bar) {
                      chart_bar.resize();
                    }
                  }
                });
                resizeObserver.observe(element_bar);
            }, millis);
          "#
        );
      }
    }
  }
}

#[component]
pub fn Plot3D(latency_by_avl_trade: ReadOnlySignal<BTreeMap<(i64, i64), f64>>) -> Element {

  let mut echarts_instance: Signal<Option<charming::Echarts>> = use_signal(||None);
  
  let mount_code = format!(
    r#"
      var millis = 150;
      setTimeout(function() {{
        const element_bar3d = document.getElementById("{CANVAS_ID_BAR3D}");
        if (!element_bar3d) {{console.log('no element_bar3d found');}}
        var chart_3d = echarts.init(element_bar3d, null, {{ width: null, height: 300 }});

        const resizeObserver = new ResizeObserver(entries => {{
          for (const entry of entries) {{
            if (entry.target === element_bar3d) {{
              chart_3d.resize();
            }} 
          }}
        }});
        resizeObserver.observe(element_bar3d);

        chart_3d.setOption({{ 
          title : {{
            text: "Avg. latency by trades and AVL rebalances",
            textStyle: {{ 
              color: 'rgba(255, 255, 255, 1)',
              fontFamily: 'Arial',
              fontSize: 18
            }},
            padding: [12, 5, 6, 13]
          }},
          backgroundColor: 'rgba(41,52,65,1)',
          color: ['#F9713C'],
          animationDuration: 0,
          animationDurationUpdate: 300,
          tooltip: {{
            formatter: function (params) {{
              var data = params.data;
              return '- Latency: ' + data[2] + ' ns'+ '<br/>' + '- Trades: '+ data[0] + '<br/>' + '- Rebalances: ' + data[1]; 
            }}
          }},
          grid3D: {{
            boxDepth: 130,
            viewControl: {{
              beta: 15,
              distance: 250,
              maxDistance: 450
            }},
            axisLine: {{
              lineStyle: {{
                color: '#262626',
                width: 1.4
              }},
            }},
            splitLine : {{
              lineStyle: {{ color: '#737373' }}
            }},
            axisPointer: {{
              lineStyle: {{ color: '#d9d9d9' }}
            }},
            light: {{
              main: {{
                color: '#f2f2f2',
                intensity: 1
              }}
            }}
          }},
          xAxis3D: {{
            name: "Trades",
            type: "category",
            data: [0, 1, 2, 3, 4, 5, 6, 7],
            nameTextStyle: {{
              color: '#ffffff',
              fontFamily: 'Consolas',
              fontSize: 14
            }},
            nameGap: 25,
            axisLabel: {{
              interval: 0,
              textStyle: {{
                color: '#cccccc'
              }}
            }},
          }},
          yAxis3D: {{
            name: "AVL rebalances",
            type: "category",
            data: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            nameTextStyle: {{
              color: '#ffffff',
              fontFamily: 'Consolas',
              fontSize: 14
            }},
            nameGap: 25,
            axisLabel: {{
              interval: 1,
              textStyle: {{
                color: '#cccccc'
              }}
            }},
          }},
          zAxis3D: {{
            type: "value",
            name: "Latency",
            nameTextStyle: {{
              color: '#ffffff',
              fontFamily: 'Consolas',
              fontSize: 14
            }},
            nameGap: 40,
            axisLabel: {{
              textStyle: {{
                color: '#cccccc'
              }}
            }},
            max: 100000
          }}
        }});
        window.rtChart3dInstance = chart_3d;
      }}, millis);
    "#
  );

  use_effect(move || {
    let data = bar3d_data(&latency_by_avl_trade());
    let chart = Chart::new()
                    .grid3d(Grid3D::new())
                    .x_axis3d(
                      Axis3D::new()
                      .type_(AxisType::Category)
                    )
                    .y_axis3d(
                      Axis3D::new()
                      .type_(AxisType::Category)
                    )
                    .z_axis3d(
                      Axis3D::new()
                    )
                    .series(
                      Bar3d::new()
                      // .name("Latency")
                      .shading("lambert")
                      .data(data)
                    );
    
    if let Some(echarts) = echarts_instance.read_unchecked().as_ref() {
      //info!("updating rt 3d");
      WasmRenderer::update(echarts, &chart);
    }
  });

  rsx! {
    div { 
      id: CANVAS_ID_BAR3D,
      onmounted: move |_evt| {
        let value = mount_code.clone();
        async move { 
          document::eval(&value[..]);
          async_std::task::sleep(Duration::from_millis(190)).await;
          let target = web_sys::window().expect("global window should exist!");
          let instance = target.get("rtChart3dInstance").expect("failed to get rt 3d chart instance");
          let js_instance = Into::<JsValue>::into(instance);
          echarts_instance.set(Some(js_instance.into()));
        }
      }
    }
  }
}