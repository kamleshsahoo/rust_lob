#![allow(non_snake_case)]

mod pages;
mod components;
mod utils;

use dioxus::prelude::*;
// use dioxus_router::prelude::*;

use pages::simulator::Simulator;

/*
Test Deps
plotters = "0.3.7"
plotters-backend = "0.3.7"
web-sys = { version = "0.3.77", features = [ "HtmlCanvasElement", "ResizeObserver", "ResizeObserverSize", "ResizeObserverEntry"] }
plotters-canvas = "0.3.0"
dioxus-resize-observer = "0.3.0"
dioxus-use-mounted = "0.3.0"

*/



#[derive(Routable, Clone)]
enum Route {
    #[route("/")]
    Simulator {},
    #[route("/:..route")]
    PageNotFound { route: Vec<String> }
}

fn main() {
    dioxus::launch(App);
}


fn App() -> Element {
    rsx! { Router::<Route> {} }
}

#[component]
fn PageNotFound(route: Vec<String>) -> Element {
    rsx! {
        h1 { "Page not found" }
        p { "We are terribly sorry, but the page you requested doesn't exist." }
        pre { color: "red", "log:\nattemped to navigate to: {route:?}" }
    }
}


