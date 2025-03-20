#![allow(non_snake_case)]
mod pages;
mod components;
mod utils;

use components::{accordion::SimLayout, template::Template};
use dioxus::prelude::*;
use pages::{simulator::Simulator, home::Home, docs::{EngineDocs, BackDocs, FrontDocs}};

#[derive(Routable, PartialEq, Clone)]
enum Route {
    #[layout(Template)]
    #[route("/")]
    Home {},
    #[nest("/docs")]
        #[route("/")]
        EngineDocs {},
        #[route("/backend")]
        BackDocs {},
        #[route("/frontend")]
        FrontDocs {},
    #[end_nest]
    #[layout(SimLayout)]
        #[route("/simulator")]
        Simulator {},
    #[end_layout]
    #[route("/:..route")]
    PageNotFound { route: Vec<String> }
}

fn main() {
    dioxus::launch(App);
}

fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}

#[component]
fn PageNotFound(route: Vec<String>) -> Element {
    rsx! {
        h1 { "Page not found" }
        p { "We are terribly sorry, but the page you requested doesn't exist." }
        pre { color: "red", "log:\nattemped to navigate to: {route:?}" }
    }
}