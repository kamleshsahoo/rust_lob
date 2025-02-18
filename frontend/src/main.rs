#![allow(non_snake_case)]
mod pages;
mod components;
mod utils;

use components::nav::NavBar;
use dioxus::prelude::*;
use pages::{simulator::Simulator, home::Home};

#[derive(Routable, PartialEq, Clone)]
enum Route {
    #[layout(NavBar)]
    #[route("/")]
    Home {},
    #[route("/simulator")]
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