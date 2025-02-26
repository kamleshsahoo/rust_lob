#![allow(non_snake_case)]
use std::collections::{BTreeMap, HashMap};
use charming::{component::{Axis, Axis3D, Grid, Grid3D, Title}, element::{AxisLabel, AxisType, Color, ColorBy, LineStyle, NameLocation, SplitLine, TextStyle, Tooltip}, series::{Bar, Bar3d}, theme::Theme, Chart, WasmRenderer};
use dioxus::prelude::*;
use crate::utils::enginestats::{bar3d_data, bin_data};
use crate::pages::simulator::PlotPropsState;

static  CANVAS_ID_HIST: &str = "rt-latency-hist";
static  CANVAS_ID_BAR: &str = "rt-latency-bar";
static  CANVAS_ID_BAR3D: &str = "rt-latency-3d";

#[component]
pub fn HistPlotCharming(latency: ReadOnlySignal<Vec<i64>>) -> Element {

  let renderer = use_signal(|| WasmRenderer::new_opt(None, Some(300)));
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
      
      .formatter(r#"a:{a} b:{b} c:{c}"#)
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
      .name_gap(42.0)
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

    let _z = renderer.read_unchecked().render(CANVAS_ID_HIST, &chart).expect("failed to create charming hist plot!");

  });

  /*
  window.addEventListener('resize', function() {{
                  chart_hist.resize();
  }});
  */
  
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
  let renderer = use_signal(|| WasmRenderer::new_opt(None, Some(250)).theme(Theme::Halloween));
  let latency_lim = use_context::<PlotPropsState>().latency_cutoff;
  //let y_max = 1.05 * (latency_lim() as f64);
  let y_max = latency_lim();

  use_effect(move || {
      
    let x_labels = vec!["ADD", "MODIFY", "CANCEL"];
    let mut avg_lat: Vec<i64> = vec![];

    //info!("avg lat for bar plot: {:?}", latency_by_ordertype());
    for (_idx, k) in x_labels.iter().enumerate() {
      if let Some(vec) = latency_by_ordertype().get(*k) {
        // NOTE: last element is the mean. See enginestats.rs 
        let avg = *vec.last().expect("failed to get last element as mean latency by ordertype!");
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

  /*
  window.addEventListener('resize', function() {{
                  chart_bar.resize();
                }})
  */
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
  
  /*
  window.addEventListener('resize', function() {{
          chart_3d.resize();
  }});
  */
  let mount_code = format!(
    r#"
      var millis = 200;
      setTimeout(function() {{
        const element_bar3d = document.getElementById("{CANVAS_ID_BAR3D}");
        if (!element_bar3d) {{console.log('no element_bar3d found');}}
        var chart_3d = echarts.init(element_bar3d);
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
            text: "Latency by no. of trades\nand AVL rebalances",
            textStyle: {{ 
              color: 'rgba(255, 255, 255, 1)',
              fontFamily: 'monospace',
              fontSize: 20
            }}
          }},
          backgroundColor: 'rgba(41,52,65,1)',
          color: ['#F9713C'],
          animationDuration: 0,
          animationDurationUpdate: 300,
          tooltip: {{ }},
          grid3D: {{
            axisLine: {{
              lineStyle: {{
                color: '#262626'
              }}
            }},
            splitLine: {{
              lineStyle: {{
                color: '#a6a6a6',
                width: 1
              }}
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
              fontFamily: 'Courier New',
              fontSize: 14
            }},
            nameGap: 25,
            axisLabel: {{
              interval: 0,
              textStyle: {{
                color: '#b3b3b3'
              }}
            }},
          }},
          yAxis3D: {{
            name: "AVL rebalances",
            type: "category",
            data: [0, 1, 2, 3, 4, 5, 6, 7],
            nameTextStyle: {{
              color: '#ffffff',
              fontFamily: 'Courier New',
              fontSize: 14
            }},
            nameGap: 25,
            axisLabel: {{
              interval: 0,
              textStyle: {{
                color: '#b3b3b3'
              }}
            }},
          }},
          zAxis3D: {{
            type: "value",
            name: "Latency",
            nameTextStyle: {{
              color: '#ffffff',
              fontFamily: 'Courier New',
              fontSize: 14
            }},
            nameGap: 40,
            axisLabel: {{
              textStyle: {{
                color: '#b3b3b3'
              }}
            }},
            max: 50000
          }}
        }});
      }}, millis);
    "#
  );

  let renderer = use_signal(|| WasmRenderer::new_opt(None, Some(250)));

  use_effect(move || {
    let data = bar3d_data(&latency_by_avl_trade());
    let chart = Chart::new()
                    .grid3d(Grid3D::new())
                    // .tooltip(Tooltip::new())
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
                      .name("Latency")
                      .shading("lambert")
                      .data(data)
                    );

    renderer.read_unchecked().render("rt-latency-3d", &chart).expect("failed to create charming bar plot!");

  });

  rsx! {
    div { 
      id: CANVAS_ID_BAR3D,
      onmounted: move |_evt| {document::eval(&mount_code);}
    }
  }
}