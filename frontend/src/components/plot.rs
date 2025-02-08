#![allow(non_snake_case)]

// use std::rc::Rc;

use std::collections::HashMap;
//"https://cdn.jsdelivr.net/npm/echarts-stat@1.2.0/dist/ecStat.min.js",

use charming::{component::{Axis, Grid}, datatype::{CompositeValue, DataPoint, Dataset, NumericValue, Transform}, df, element::{AxisPointer, AxisPointerType, AxisType, BackgroundStyle, Formatter, Tooltip, Trigger}, series::Bar, theme::Theme, Chart, WasmRenderer};
//use dioxus::{hooks::use_signal, prelude::{rsx, spawn, Element, Props}};
use dioxus::{logger::tracing::{info, warn}, prelude::*, web::WebEventExt};
// use dioxus_use_mounted::use_mounted;
// use plotters::{prelude::IntoDrawingArea, style::{RED, WHITE}};
use plotters_canvas::CanvasBackend;
use plotters::prelude::*;
// use web_sys::{wasm_bindgen::JsCast, HtmlCanvasElement, ResizeObserver, ResizeObserverEntry};
use web_sys::{wasm_bindgen::{prelude::Closure, JsCast, JsValue}, window, HtmlCanvasElement};
// use dioxus_resize_observer::{use_resize, use_size};

use crate::PlotPropsState;
// use web_sys::window;


static  CANVAS_ID_HIST: &str = "latency-hist";
static  CANVAS_ID_BAR: &str = "latency-bar";

// #[derive(PartialEq, Props, Clone)]
// pub struct LatencyProps {
//     latencies: Vec<u128>,
// }


/*Initial Working Version
pub fn LatencyHistogram(props: LatencyProps) -> Element {
  let latency = props.latencies;
  let x_lim = use_context::<PlotPropsState>().xlim_hist;
  let y_lim = use_context::<PlotPropsState>().ylim_hist;
  // let current_latency_length = latency.len();
  // info!("latency len: {:?}", current_latency_length);

  // use_effect(move || {});
  
  // let x_lim = props.x_lim;
  let mut canvas_element = use_signal(||None);

 
  if latency.len() > 0 {
    //info!("xlim: {:?}, ylim: {:?}", x_lim(), y_lim());
    if let Some(canvas) = canvas_element() {
      //let ctx = canvas.get_context("2d");

      // let eval = document::eval(r#"
      // const canvas = document.querySelector('#latency-plot');

      // const observer = new ResizeObserver((entries) => {
      // const entry = entries.find((entry) => entry.target === canvas);
      // canvas.width = entry.devicePixelContentBoxSize[0].inlineSize;
      // canvas.height = entry.devicePixelContentBoxSize[0].blockSize;
      //   /* … render to canvas … */
      // });

      // observer.observe(canvas, {box: ['device-pixel-content-box']});
      // "#);

      let backend = CanvasBackend::with_canvas_object(canvas).expect("error creating plotter html canvas backend");
      let root = backend.into_drawing_area();
      root.fill(&WHITE).expect("error filling white");
      // let unicode = 0x2076; // U+207x
      // let c = char::from_u32(unicode).unwrap();
      // let sup = c.to_string(); 

      let mut chart = ChartBuilder::on(&root)
      .x_label_area_size(35)
      .y_label_area_size(45)
      .margin(15)
      .caption("Order latency histogram", ("sans-serif", 15.0).into_font())
      .build_cartesian_2d((0..x_lim()).into_segmented(), 0..y_lim()).expect("failed initializing chart of latency hist");

      chart.configure_mesh()
      // .x_labels(4)
      //.disable_x_mesh()
      //.bold_line_style(WHITE.mix(0.3))
      .x_desc("Latency ( ns )")
      //.y_desc(format!("Number of Orders ( x 10{} )",  sup))// .y_labels(4)
      .y_desc("Number of Orders")// .y_labels(4)
      .axis_desc_style(("sans-serif", 12))
      // .y_label_formatter(&|t| format!("{:.1?}", *t as f64/1000.0))
      .draw().expect("configure chart failed for latency hist!");

      let x_hist = Histogram::vertical(&chart)
      .style(RED.filled())
      //.margin(10)
      //.data(random_points.iter().map(|(x, _)| (*x, 1)));
      .data(latency.iter().map(|x| (*x, 1)));

      let _d = chart.draw_series(x_hist).unwrap();
      // To avoid the IO failure being ignored silently, we manually call the present function
      root.present().expect("Unable to write to canvas, please make sure canvas for hist exists");
      // });
    }
  }


  
  rsx! {
    canvas {
      id: CANVAS_ID, 
      //background_color: "white",
      class: "canvas",
      width: "640",
      height: "360",
      onmounted: move|element| {
        let web_sys_element = element.as_web_event();
        canvas_element.set(Some(web_sys_element.dyn_into::<HtmlCanvasElement>().expect("fetching canvas for latency hist failed!")));
      }
    }
  }
}
*/

// const DEBOUNCE_MS: i32 = 200;
// const MIN_WIDTH: u32 = 300;
// const MIN_HEIGHT: u32 = 200;
// const ASPECT_RATIO: f64 = 16.0 / 9.0;

#[component]
pub fn LatencyHistogramv2(latency: Signal<Vec<i64>>) -> Element {
    let mut canvas_element = use_signal(|| None::<HtmlCanvasElement>);
    //let mut screen_width = use_signal(|| 0);
    // let mut canvas_width = use_signal(|| 0);
    // let mut canvas_height = use_signal(|| 0);


    // Draw histogram
    use_effect(move || {
        if latency.len() > 0 {
          // info!("lat effect");
            if let Some(canvas) = canvas_element() {
              draw_histogram(canvas, latency);
            }
        }
    });

    // use_effect(move || {
    //   if let Some(window) = window() {
    //     let width = window.inner_width().expect("window should have inner width").as_f64().expect("failed to parse Js value to f64") as u32;
    //     // screen_width.set(width);
    //     info!("screen resize trigg");
    //     let (canvas_w, canvas_h) = match width {
    //       ..600 => (1200, 675),
    //       600..800 => (1600, 900),
    //       800..1200 => (2000,1125),
    //       _ => (2400,1350)
    //     };
    //     canvas_width.set(canvas_w);
    //     canvas_height.set(canvas_h);
    //   }
    // });

    
    rsx! {
      canvas {
        // class: "my-canvas",
        id: "latency-hist-plotter",
        width: "2250",
        height: "1265",
        onmounted: move |element| {
            let web_sys_element = element.as_web_event();
            canvas_element.set(Some(
                web_sys_element
                    .dyn_into::<HtmlCanvasElement>()
                    .expect("fetching canvas for latency hist failed!")
            ));
          }
        },  
      }
}

fn draw_histogram(canvas: HtmlCanvasElement, latency: Signal<Vec<i64>>) {
  let x_lim = use_context::<PlotPropsState>().latency_cutoff; 
  let y_lim = use_context::<PlotPropsState>().frequency_cutoff;

  let backend = CanvasBackend::with_canvas_object(canvas).expect("error creating plotter html canvas backend");
  let root = backend.into_drawing_area();
  root.fill(&WHITE).expect("error filling white");

  let mut chart = ChartBuilder::on(&root)
      // .x_label_area_size(67)
      // .y_label_area_size(72)
      .set_left_and_bottom_label_area_size(125)
      .margin(60)
      // .caption("Order latency histogram",("sans-serif", 40.0).into_font(),)
      .build_cartesian_2d((0..x_lim()).into_segmented(), 0..y_lim())
      .expect("failed initializing chart of latency hist");

  chart
      .configure_mesh()
      .x_desc("Latency (ns)")
      .y_desc("Number of Orders")
      .y_label_style(("sans-serif", 38))
      .x_label_style(("sans-serif", 42))
      //.axis_desc_style(("sans-serif", 45))
      .draw()
      .expect("configure chart failed for latency hist!");

  let x_hist = Histogram::vertical(&chart)
      .style(RED.filled())
      .data(latency.iter().map(|x| (*x, 1)));

  let _d = chart.draw_series(x_hist).unwrap();
  root.present()
      .expect("Unable to write to canvas, please make sure canvas for hist exists");
}

/*Working version v2
fn get_canvas_dimensions(screen_size: ScreenSize) -> (u32, u32) {
  match screen_size {
      ScreenSize::Small => (400, 200),   // Example dimensions for small screens
      ScreenSize::Medium => (600, 300),  // Example dimensions for medium screens
      ScreenSize::Large => (800, 400),   // Example dimensions for large screens
  }
}

/// Enum to represent screen size categories.
#[derive(Clone, Copy)]
enum ScreenSize {
  Small,
  Medium,
  Large,
}

fn get_screen_size() -> ScreenSize {
  let window = web_sys::window().expect("No global `window` found");
  let width = window.inner_width().unwrap().as_f64().unwrap();
  if width < 599.0 {
      ScreenSize::Small
  } else if width >= 600.0 && width <= 800.0 {
      ScreenSize::Medium
  } else {
      ScreenSize::Large
  }
}


pub fn LatencyHistogramv2(props: LatencyProps) -> Element {
  let latency = props.latencies;
  let x_lim = use_context::<PlotPropsState>().xlim_hist;
  let y_lim = use_context::<PlotPropsState>().ylim_hist;
  // let current_latency_length = latency.len();
  // info!("latency len: {:?}", current_latency_length);

  // use_effect(move || {});
  
  // let x_lim = props.x_lim;
  let mut screen_size = use_signal(|| get_screen_size());
  let mut canvas_element: Signal<Option<HtmlCanvasElement>> = use_signal(||None);
  // let mut mt = use_mounted();
  // let size = use_size(mt);
  // let z = mt;

   // Track screen size changes
   use_effect(move || {
    let window = web_sys::window().expect("No global `window` found");
    let closure = Closure::<dyn FnMut()>::new(move || {
        screen_size.set(get_screen_size());
    });
    window
        .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
        .expect("Failed to add resize event listener");
    closure.forget();
  });

  // use_effect(move||{ 
    if latency.len() > 0 {
      //info!("xlim: {:?}, ylim: {:?}", x_lim(), y_lim());
      
      if let Some(canvas) = canvas_element() {
        
        
        let (width, height) = get_canvas_dimensions(screen_size());
        info!("current width: {:?} current height: {:?}", width, height);
        
        canvas.set_width(width);
        canvas.set_height(height);

        let backend = CanvasBackend::with_canvas_object(canvas).expect("error creating plotter html canvas backend");
        let root = backend.into_drawing_area();
        root.fill(&WHITE).expect("error filling white");
     
        let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(45)
        .margin(15)
        .caption("Order latency histogram", ("sans-serif", 15.0).into_font())
        .build_cartesian_2d((0..x_lim()).into_segmented(), 0..y_lim()).expect("failed initializing chart of latency hist");

        chart.configure_mesh()
        .x_desc("Latency ( ns )")
        .y_desc("Number of Orders")
        .axis_desc_style(("sans-serif", 12))
        .draw().expect("configure chart failed for latency hist!");

        let x_hist = Histogram::vertical(&chart)
        .style(RED.filled())
        .data(latency.iter().map(|x| (*x, 1)));

        let _d = chart.draw_series(x_hist).unwrap();
        // To avoid the IO failure being ignored silently, we manually call the present function
        root.present().expect("Unable to write to canvas, please make sure canvas for hist exists");
      } else {
        warn!("canvas for histogram not found");
      }
    }
  // });//

  
  rsx! {
    canvas {
      id: CANVAS_ID, 
      //background_color: "white",
      class: "canvas",
      width: "640",
      height: "360",
      // onmounted: move|element| {
      //   let web_sys_element = element.as_web_event();
      //   canvas_element.set(Some(web_sys_element.dyn_into::<HtmlCanvasElement>().expect("fetching canvas for latency hist failed!")));
      // }
      onmounted: move|event| {
        // mt.onmounted(event);
        let web_sys_element = event.as_web_event();
        canvas_element.set(Some(web_sys_element.dyn_into::<HtmlCanvasElement>().expect("fetching canvas for latency hist failed!")));
      },
    }
  }
}
*/

#[component]
pub fn BarPlot(latency_by_ordertype: Signal<HashMap<String, Vec<f64>>>) -> Element {
  
  let mut canvas_element = use_signal(|| None::<HtmlCanvasElement>);

  use_effect(move || {
    if latency_by_ordertype().len() > 0 {
      if let Some(canvas) = canvas_element() {
        draw_bar_with_err_bars(canvas, latency_by_ordertype());
      }
    }
  });

  
  rsx! {
    canvas {
      id: "latency-bar-plotters",
      width: "1600",
      height: "900",
      onmounted: move |element| {
          let web_sys_element = element.as_web_event();
          canvas_element.set(Some(
              web_sys_element
                  .dyn_into::<HtmlCanvasElement>()
                  .expect("fetching canvas for latency hist failed!")
          ));
        }
      },  
    }
}

fn draw_bar_with_err_bars(canvas: HtmlCanvasElement, latency_by_ordertype: HashMap<String, Vec<f64>>) {
  
  //TODO: check if this needs to be dynamic for eg a ordertype is not present
  let x_labels = ["ADD", "MODIFY", "CANCEL"];
  let latency_lim = use_context::<PlotPropsState>().latency_cutoff;
  let y_max = 1.05 * (latency_lim() as f64);
  // let y_max = latency_by_ordertype.iter().map(|(_k,v)| v[1]).reduce(|acc, e| acc.max(e)).expect("failed to get max avg latency by order type!")*1.05;
  // info!("current lat stats: {:?}\ny_max: {:?}", latency_by_ordertype, y_max);

  let backend = CanvasBackend::with_canvas_object(canvas).expect("error creating plotter html canvas backend");
  let root = backend.into_drawing_area();
  root.fill(&WHITE).expect("error filling white");

  let mut ctx = ChartBuilder::on(&root)
  // .set_label_area_size(LabelAreaPosition::Left, 40)
  // .set_label_area_size(LabelAreaPosition::Bottom, 40)
  .set_left_and_bottom_label_area_size(125)
  .margin(60)
  //.caption("Bar Demo", ("sans-serif", 40))
  //.build_cartesian_2d((0..x_labels.len()-1).into_segmented(), 0.0..80.0)
  .build_cartesian_2d(x_labels.into_segmented(), 0.0..y_max)
  .expect("failed initializing bar plot");
    
  ctx.configure_mesh()
  // .x_labels(4)
  .x_desc("Order Type")
  .y_desc("Latency (ns)")
  .y_label_style(("sans-serif", 38))
  .x_label_style(("sans-serif", 42))
  .draw()
  .expect("configure chart failed for latency barplot with err bars!");

  let mut bar = Vec::new();
  let mut err_bar = Vec::new();

  for (idx, k) in x_labels.iter().enumerate() {
    if let Some(vec) = latency_by_ordertype.get(*k) {
      let avg = *vec.last().expect("failed to get avg as last elem");
      let x0 = SegmentValue::Exact(k);
      let x1 = if idx == 2 {SegmentValue::Last} else {SegmentValue::Exact(&x_labels[idx+1])};
      let x0_err = SegmentValue::CenterOf(k);

      let mut rect = Rectangle::new([(x0, 0.0), (x1, avg)], RED.filled());
      rect.set_margin(0, 0, 25, 25);

      let err = ErrorBar::new_vertical(x0_err, vec[0], avg, vec[1], BLUE.stroke_width(5), 10);

      bar.push(rect);
      err_bar.push(err);
    }
  }
  ctx.draw_series(bar).expect("failed to draw bar");
  ctx.draw_series(err_bar).expect("failed to draw err bar");

  root.present().expect("Unable to write to canvas, please make sure canvas for barplot with err bar exists!");

}

#[component]
pub fn BarPlotCharming(latency_by_ordertype: ReadOnlySignal<HashMap<String, Vec<f64>>>) -> Element {
  let renderer = use_signal(|| WasmRenderer::new(500, 281).theme(Theme::Halloween));
  let latency_lim = use_context::<PlotPropsState>().latency_cutoff;
  let y_max = 1.05 * (latency_lim() as f64);

  use_effect(move || {
    if latency_by_ordertype().len() > 0 {
      
      let x_labels = vec!["ADD", "MODIFY", "CANCEL"];
      let mut avg_lat: Vec<f64> = vec![];
      
      for (idx, k) in x_labels.iter().enumerate() {
        if let Some(vec) = latency_by_ordertype().get(*k) {
          let avg = *vec.last().expect("failed to get avg as last elem");
          avg_lat.push(avg);
        }
      }


      let chart = Chart::new().x_axis(
        Axis::new().type_(AxisType::Category).data(x_labels)
      ).y_axis(Axis::new().type_(AxisType::Value).max(y_max)).series(
        Bar::new()
        // .show_background(true).background_style(BackgroundStyle::new().color("rgba(180, 180, 180, 0.2)"))
        .data(avg_lat)
      );

      renderer.read_unchecked().render(CANVAS_ID_BAR, &chart).expect("failed to create charming bar plot!");

    } 
  });

  rsx! {
    div {
      id: CANVAS_ID_BAR
    }
  }
}

#[derive(Debug)]
struct Bin {
    range: BinRange,
    count: usize,
}


#[derive(Debug)]
enum BinRange {
    Fixed(i64, i64),  // [lower, upper)
    CatchAll(i64),    // [max, inf)
}

fn bin_data(data: Vec<i64>, bin_width: i64, max_value: i64) ->  Vec<DataPoint> {

    assert_eq!(false, data.is_empty(), "data for binning should not be empty!");

    let num_bins: i64 = max_value/bin_width;
    let mut bins = HashMap::new();

    for &value in &data {
      if value >= max_value {
        *bins.entry(num_bins).or_insert(0) += 1;
        continue;
      }
      let bin_index = value/bin_width;
      *bins.entry(bin_index).or_insert(0) += 1;
    }

    let mut result = Vec::new();
    for (bin_index, count) in bins {
      let range = if bin_index == num_bins {
        BinRange::CatchAll(max_value)
    } else {
        let lower = bin_index * bin_width;
        let upper = lower + bin_width;
        BinRange::Fixed(lower, upper)
    };

    result.push(Bin { range, count });
    }

  result.sort_by_key(|bin| match bin.range {
    BinRange::Fixed(lower, _) | BinRange::CatchAll(lower) => lower
  });
 

  // let _u: Vec<_> = result.drain(num_bins as usize..).collect();
  // info!("excluded bin: {:?}", _u);
  
  let processed_bins = result.into_iter().filter_map(|bin| {
    match bin.range {
      BinRange::Fixed(x0, x1) => {
        let centre: f64 = (x0 + x1) as f64 / 2.0;
        let label = format!("{}-{}", x0, x1);
      
        Some(DataPoint::Value(CompositeValue::Array(vec![
          CompositeValue::Number(NumericValue::Float(centre)), 
          CompositeValue::Number(NumericValue::Integer(bin.count as i64)),
          CompositeValue::Number(NumericValue::Integer(x0)),
          CompositeValue::Number(NumericValue::Integer(x1)),
          CompositeValue::String(label)
        ])))      
        },
        BinRange::CatchAll(_) => None
      }
    }
  ).collect::<Vec<_>>();

  processed_bins

}


#[component]
pub fn HistPlotCharming(latency: ReadOnlySignal<Vec<i64>>) -> Element {

  let renderer = use_signal(|| WasmRenderer::new(640, 360));
  let latency_lim = use_context::<PlotPropsState>().latency_cutoff;
  let freq_lim = use_context::<PlotPropsState>().frequency_cutoff;
  let mut chart_instance: Signal<Option<charming::Echarts>> = use_signal(||None);

  use_effect(move || {
    let window = web_sys::window().expect("no global window find");
    let closure = Closure::<dyn FnMut()>::new(move || {
      if let Some(instance) = chart_instance.read_unchecked().as_ref() {
        // let opts = to_val;
        instance.resize(JsValue::null());
      }
    });
    window.add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref()).expect("Failed to add resize event listener for charming histogram");
    closure.forget();
  });

  use_effect(move || {
    if latency.len() > 0 {
      let max_latency = latency_lim();
      let max_freq = freq_lim();
      let latency_bin_width: i64 = if max_latency < 20_000 { 250 } else { 500 };
      //let x_labels = (0..max_latency).step_by(latency_bin_width as usize).collect::<Vec<i64>>();
      
      let binned_data = bin_data(latency(), latency_bin_width, max_latency);
      //info!("binned data: {:?}", &binned_data);
      
      let chart = Chart::new()
      //see if Tooltip,  legend, grid required
      .tooltip(
        Tooltip::new()
        .trigger(Trigger::Axis)
        .axis_pointer(
          AxisPointer::new().type_(AxisPointerType::Shadow)
        )
        // .formatter(r#"
        // function (params) {
        // var tar = params[1];
        // return tar.name + '<br/>' + tar.seriesName + ' : ' + tar.value;
        // }
        // "#)
        //.formatter(r#"{b0}<br />{a0}: {c1}"#)
        //{a} for series name, {b} for category name, {c} for data value, {d} for none;
      )
      .grid(
        Grid::new()
        // .left("3%")
        .right("4%")
        // .bottom("3%")
        .contain_label(true)
      )
      
      .x_axis(
        Axis::new().scale(true)
      ).y_axis(Axis::new().type_(AxisType::Value).max(max_freq))
      .series(
        Bar::new()
        .name("Count:")
        .data(binned_data)
        // .show_background(true).background_style(BackgroundStyle::new().color("rgba(180, 180, 180, 0.2)"))
        // .data(avg_lat)
      );

      if let Ok(echarts_instance) = renderer.read_unchecked().render(CANVAS_ID_HIST, &chart) {
        chart_instance.set(Some(echarts_instance));
      };

    } 
  });

  rsx! {
    div {
      id: CANVAS_ID_HIST
    }
  }
}