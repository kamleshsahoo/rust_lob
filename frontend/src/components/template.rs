use dioxus::prelude::*;
use crate::Route;

// const MENU_ICON: &str = r#"
// <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-list" viewBox="0 0 16 16">
//   <path fill-rule="evenodd" d="M2.5 12a.5.5 0 0 1 .5-.5h10a.5.5 0 0 1 0 1H3a.5.5 0 0 1-.5-.5m0-4a.5.5 0 0 1 .5-.5h10a.5.5 0 0 1 0 1H3a.5.5 0 0 1-.5-.5m0-4a.5.5 0 0 1 .5-.5h10a.5.5 0 0 1 0 1H3a.5.5 0 0 1-.5-.5"/>
// </svg>
// "#;

#[component]
pub fn Template() -> Element {
  static CSS: Asset = asset!("assets/template.css");

  rsx! {
    document::Stylesheet {href: CSS},
    Header { }
    Outlet::<Route> {}
    Footer { }
  }
}


#[component]
fn Header() -> Element {
  rsx!{
    nav {
      div {
        class: "nav-container",
        Link {
          class: "logo",
          active_class: "nav-active",
          to: Route::Home { },
          "Home",
        }
        button { 
          class: "menu-button",
          span {
            class: "menu-icon",
            svg {
              class: "menu-icon-svg",
              xmlns: "http://www.w3.org/2000/svg",
              view_box: "0 0 24 24",
              path {
                d: "M6 12H18",
                stroke: "currentcolor",
                stroke_linecap: "round"
              }
              path {
                d: "M6 15.5H18",
                stroke: "currentcolor",
                stroke_linecap: "round"
              }
              path {
                d: "M6 8.5H18",
                stroke: "currentcolor",
                stroke_linecap: "round"
              }
            }
          }
        },
        div {
          class: "nav-links",
          Link {
            active_class: "nav-active",
            to: Route::Simulator { },
            "Simulator"
          },
          Link {
            active_class: "nav-active",
            to: Route::Docs {  },
            "Documentation"
          },
        }
      }
    }
  }
}

#[component]
fn Footer() -> Element {
  rsx!{
    footer {
      div {
        class: "footer-container",
        div { 
          class: "copyright",
          p { "© 2025 Kamlesh Sahoo" }
          // p { "All rights reserved" }
        },
        div {
          class: "social-links",
          a { 
            href: "mailto:kamlesh.sahoo20@gmail.com",
            class: "social-link",
            title: "Email",
            svg {
              xmlns: "http://www.w3.org/2000/svg",
              width: "24",
              height: "24",
              view_box: "0 0 24 24",
              fill: "none",
              stroke: "currentcolor",
              stroke_width: "2",
              stroke_linecap: "round",
              stroke_linejoin: "round",
              path {
                d: "M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"
              }
              polyline { 
                points: "22,6 12,13 2,6"
              }
            }
          }
          a { 
            href: "https://github.com/kamleshsahoo",
            target: "_blank",
            class: "social-link",
            title: "Github",
            svg {
              xmlns: "http://www.w3.org/2000/svg",
              width: "24",
              height: "24",
              view_box: "0 0 24 24",
              fill: "none",
              stroke: "currentcolor",
              stroke_width: "2",
              stroke_linecap: "round",
              stroke_linejoin: "round",
              path {
                d: "M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"
              }
            }
          }
          a { 
            href: "https://www.linkedin.com/in/kamlesh-sahoo",
            target: "_blank",
            class: "social-link",
            title: "LinkedIn",
            svg {
              xmlns: "http://www.w3.org/2000/svg",
              width: "24",
              height: "24",
              view_box: "0 0 24 24",
              fill: "none",
              stroke: "currentcolor",
              stroke_width: "2",
              stroke_linecap: "round",
              stroke_linejoin: "round",
              path {
                d: "M16 8a6 6 0 0 1 6 6v7h-4v-7a2 2 0 0 0-2-2 2 2 0 0 0-2 2v7h-4v-7a6 6 0 0 1 6-6z"
              }
              rect { 
                x: "2",
                y: "9",
                width: "4",
                height: "12"
              }
              circle { 
                cx: "4",
                cy: "4",
                r: "2"
              }
            }
          }
        }
      }
    }
  }
}