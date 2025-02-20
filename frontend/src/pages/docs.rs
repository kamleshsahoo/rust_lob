use dioxus::prelude::*;

#[component]
pub fn Docs() -> Element {
  // const ROCKET: Asset = asset!("/assets/rocket.svg");
  static CSS: Asset = asset!("assets/docs.css");
  const RUST_ICON: Asset = asset!("/assets/rust-logo-64x64.png");

  rsx! {
    document::Stylesheet {href: CSS},
    div {
      class: "docs",
      div {
        class: "docs-header", 
        h1 { "Building a Real-Time Order Book in " },
        img { 
          src: RUST_ICON,
          alt: "Rust icon",
          class: "rust-logo"
        }
      },
      section {
        h2 { "Introduction" },
        p {
          "Recently, I’ve been diving into the world of "
          strong { "High-Frequency Trading (HFT)" } " systems, the kind used by market makers and proprietary trading firms. If you're unfamiliar, HFT is all about executing trades accurately at lightning speed, often in microseconds or even less. You can read more about it "
          a {
            href: "https://en.wikipedia.org/wiki/High-frequency_trading",
            target: "_blank",
            "here"
          }
        },
        p {
          "In this blog post, I'll walk you through how I built an "
          em {"end-to-end trading system"}
          " with a web interface. The goal? To allow users to place their orders and have them processed by a low-latency trading engine. Performance and user experience were my top priorities, so I chose tools that could deliver both speed and flexibility."
        }
      },
      section { 
        h2 { "Why Rust?" },
        p { 
          "For the backend, I chose "
          strong { "Rust" }
          ". Why? Because Rust is fast, like " 
          i {"really"} 
          " fast. Apart from being a perfect choice, for compute-intensive tasks like number crunching and large scale data manipulations, rust provides inherent memory safety with its borrow checking principles. On top of that, Rust has stellar support for async programming patterns, built on top of the "
          strong {"Tokio"}, 
          " ecosystem, which makes it ideal for building lean and mean high-performance servers. The current Rust ecosystem ranging from building professional-grade backend systems to data-intensive AI/ML applications deserves a blog of its own. But you can check out the official " 
          a {
            href: "https://doc.rust-lang.org/stable/book/",
            target: "_blank",
            "Rust"
          },
          " and ",
          a {
            href: "https://tokio.rs/tokio/tutorial",
            target: "_blank",
            "Tokio"
          },
          " book to get an idea." 
        }
        p {
          "I used "
          strong { "Axum" },
          " to build the backend server which runs the core trading engine and feeds into our frontend web application. "
          i { "Actix" },
          " is another good choice for building servers and both integrate well with Tokio, plus I happened to pick up axum earlier in my journey."
        }
      },
      section { 
        h2 { "The Frontend Dilemma" },
        p { 
          "For the frontend, I wanted something as performant as Rust but as ubiquitous as HTML and JavaScript. I’ve used frameworks like "
          strong { "Next.js"}
          " and "
          strong { "Remix" }
          " in past, but they didn't quite fit my needs. The first reason was obvious- I didn't want to give up on the performance and fun of writing Rust. Second, I recognized that  modern web applications are built on " 
          strong { "JavaScript" }
          ", "
          strong { "HTML5" }
          ", and "
          strong { "CSS" }
          ". So, anything that transpiles to these technologies can be used to build a web page."
        }
        p {
          strong { "Dioxus" }
          ", does exactly that and more! With Dioxus, I could now write my frontend entirely in Rust (okay, maybe with a sprinkle of JavaScript here and there). One of the standout features is, Dioxus compiles to web, desktop and mobile platforms, all with the same codebase. How cool is that? " 
          "Check out the "
          a {
            href: "https://dioxuslabs.com/",
            target: "_blank",
            "Dioxus docs"
          } 
          " if you're curious🚀"
        }
      },
      section {
        h2 { "The Heart of the System: The Matching Engine" }
        p {
          "At the core of any trading system is the "
          strong { "matching engine" }
          ". Think of it as a ledger that keeps track track of buy and sell orders. It updates based on market events like new orders, modifications, or deletions."
        },
        p { 
          "A "
          strong { "bid" }
          " is an order to buy an asset, while an "
          strong { "ask" }
          " is an order to sell. For example, in a stock exchange, the asset could be an Apple (AAPL) share, or on a crypto exchange like Kraken, it could be a currency pair like BTC-USD."
        },
        p {
          "Orders are executed based on two key principles:",
          ul {
            li { 
              strong { "Price Priority:" }
              " The best bid or ask gets priority."
            },
            li { 
              strong { "Time Priority (FIFO):" }
              " If two orders have the same price, the one that came first gets executed first."
            },
          }
        },
        p {
          "For this project, we respect the "
          em {"Price Priority"}
          " and use "
          em {"FIFO"}
          " as tie-breakers."
        }
      },
      section { 
        h2 { "Order Types" }
        p { "There are three fundamental order types:" }
        ol {
          li { 
            strong { "ADD:" }
            " A new order is added at a specific price and quantity."
          },
          li { 
            strong { "MODIFY:" }
            " An existing order is modified."
          },
          li { 
            strong { "DELETE:" }
            " An existing order is deleted."
          },
        }
        p {
          "Every other order type is based on these 3 fundamental order types and differs only in what condition needs to be met for a trade to take place. For example, a "
          em {"LIMIT"} 
          " buy order specifies a condition, like buying 100 shares of AAPL only if the price is $10.50 or lower. Similarly, there are "
          i {"MARKET"}
          " orders which can be executed (fulfilled) instantly at current market prices. For this project, I focused on "
          em {"LIMIT"}
          " orders, as they provide the foundation for implementing other types."
        }
      },
      section {
        h2 { "Data Structures and architecture" }
        p { 
          "To build a real-time order book capable of handling high throughput and low latency, I needed a data structure that could efficiently process at least 1 million transactions per second (TPS). Simple binary search trees or "
          i { "BST" }
          " were a natural starting point as they provide constant time lookups, but their worst case performance can degrade to "
          i { "O(n)" }
          " if the tree becomes unbalanced. A better alternative is AVL trees, a type of self-balancing BST that maintains logarithmic time complexity ("
          i { "O(log n)" }
          ") by ensuring the tree remains balanced at all times. This makes insertions, deletions, and lookups much faster and more predictable, which is crucial for a trading system where every microsecond counts. (TODO.....) Now, let's dive into how I implemented an AVL tree in Rust, overcoming challenges related to memory safety and ownership." 
        }
        h3 { "Implementing an AVL Tree in Rust" }
        p {
          "Unlike traditional implementations that use raw pointers, Rust requires safe memory management, making recursive tree data structures tricky. While Rust has " 
          a {
            href: "https://doc.rust-lang.org/book/ch15-06-reference-cycles.html",
            target: "_blank",
            "smart pointers "
          }
          "such as " 
          code { "Rc<T>" }
          " and "
          code { "RefCell<T>" }
          " to deal with recursive data structures, I quickly ran into increased code complexity. To simplify this, I opted for an " 
          em { "Arena" } 
          " based tree structure, where all nodes are stored in a collection like "
          code {"HashMap"} 
          " indexed by their prices. Instead of traditional parent-child pointers, each node carries an index to its position in the map, making traversal simple while avoiding multiple mutable borrows. This eliminated the need for "
          i { "unsafe" } 
          " code and made the implementation significantly cleaner. The approach is not something specific to rust but rather a clever data access pattern which you can find many resources about online. This "
          a { 
            href: "https://rust-leipzig.github.io/architecture/2016/12/20/idiomatic-trees-in-rust/",
            target: "_blank",
            "guide"
          }    
          " served as my starting point as it addressed similar difficulties and explained how arena-based structures can be a life-saver. The only trade-off is that every tree operation must maintain an access to this " 
          i { "arena" }
          ". However, this is a small price to pay for a cleaner, safer, and more ergonomic implementation."
        }
      },
      section { 
        h2 { "Server: Enabling Real-time trading" }
        p {
          "Alright, so now that I had an efficient order-matching engine, the next challenge was figuring out how to actually deliver real-time updates to users. After all, a trading system that only executes orders but doesn't tell anyone about it isn’t much useful. This is where the "
          strong { "backend server" } 
          " comes in. Its the orchestrator that makes real-time trading actually real-time. Here's how I built it: "
        }
        ol {  
          li { 
            strong { "WebSockets" }
            p {
              "WebSockets are a communication protocol designed for persistent, real-time data exchange between a client and a server. Unlike the standard HTTP requests (GET, POST etc), websockets keep the connection open, like an ongoing phone call. This means no constant reconnecting, no unnecessary overhead, just pure, uninterrupted data flow. Since WebSockets run on the same underlying TCP protocol as HTTP, they technically start as HTTP requests before upgrading to WebSockets via an " 
              em {"upgrade header"}
              ". For implementation, I used " 
              em {"Axum"}
              " which makes handling WebSockets in Rust a breeze. Under the hood, it relies on " 
              em {"Tungstenite"}
              ", the real websocket workhorse that does the heavy lifting."
            }
          },
          li { 
            strong { "Async Programming for high performance" }
            p {
              "As mentioned earlier in this blog, Rust’s async capabilities are top-tier, and that’s where " 
              strong { "Tokio" }
              " shines. It’s an async runtime for Rust that lets you handle network communication like TCP sockets without blocking execution. Here's how I structured things: " 
              ul { 
                li {
                  "The trading engine, built with AVL trees (covered in the previous section), runs as an " 
                  code { "async" }
                  " task"
                },
                li {
                  "This frees up the main thread to focus purely on streaming real-time updates via WebSockets." 
                },
                li {
                  "Communication between the WebSocket server and the engine happens over "
                  em {"Tokio"}
                  " "
                  strong {"channels"}
                  ", allowing for smooth message passing."
                },
              }
            }
            p {
              "This setup means trades get executed while updates are instantly streamed to users, all without blocking execution at native speeds. For context, apart from regular batching I had to throttle the server updates with "
              code { "time::sleep()" }
              " just so the UI could keep up and doesnt crash. (A rare and humbling moment for any backend engineer). The server was processing orders at an average speed of ..., which translates to ... transactions per second. If you're into real-time systems, chat apps, or just want an excuse to dive into it, check out the Tokio docs—it’s a goldmine of async wizardry."
            }
          },
        }
      }
      // section {
      //   h2 { "Wrapping Up the Backend (Or, When Your Server is Too Fast)" }
      //   p {
      //     "At this point, all that’s left was packaging our server and hosting it on a server, typically by cloud computte and storage provider like AWS or GCP. We dockerized our axum server and deployed on GCP (Google Cloud Storage). I was again pleasently sureprised by the minimal Rust image sizes of 85.7mb, which was not seen by me earlier.in the past I was used to seeing typical docker images of atleast 500-750mbs even after multistage builds. Rust compiles to a binary which can run natively meaning .exe file is all that one needs to run our app reducing the overheads a lot. Apart from that I chose to keep a couple of crucial components as a future add-on to keep things tractable. The first being upgrading our naked axum server to a professional web server, with load balancing features. Pingora is one such battle-tested rust framework which does that. Its made by Cloudfare and is part of the push towards a more memory safe alternatives of C notably by the US government as well. Do check out their docs to about the motivation behind this project. The second was a data streaming platforming with pub-sub mechanism, so that we simply do not throw away the orderbook updates as we at the end of our simulations. We need to have a way to persist the data, maintain state while streaming the updates to them in real-time. My data engineering background, was already yelling me to use kafka or atleast libC kafka implemnetaion of rust (anyone who tried to meddle with all zookeeper configs or the newer Kraft mode will know what IOm talking abouyt) I researched a bit and 2 such pure rust projects caught my eye- Fluvio and Iggy (if not anything, I love how cool rust project names are!). Fluvio is very promising lean and mean distributed data streaming engine written in Rust. This is analogous to a typical Flink + Kafka setup one might be using in real-time streaming scenarios. Fluvio provides connectors in rust, python, go etc to hook up your databases in AWS, Snowflake etc while their Dataflow engine lets you write you perform traditional transformations and run ETL jobs. Do check them out, to know more. Iggy seems a bit more recent but Im open to keep getting amazed. With all the devops and data engineering out of the window, we can proceed to build our beautiful web page."
      //   }
      //   p {
      //     "For context, apart from batching I had to throttle the server updates with " 
      //     code { "time::sleep()" }
      //    " just so the UI could keep up and doesnt crash. (A rare and humbling moment for any backend engineer). The server was processing orders at an average speed of ..., which translates to ... transactions per second. This blazing performance comes down to a combination of:"
      //    ul { 
      //     li { 
      //       strong {"AVL trees"}
      //       " for efficient order book management" },
      //     li { 
      //       strong {"WebSockets"}
      //       " for real-time, low-latency updates" },
      //     li { 
      //       strong {"Tokio async runtime"}
      //       " to keep everything running concurrently without blocking" },
      //    }
      //   }
      //   p {
      //     "Of course, a backend this fast needs a frontend that can keep up—so next, we dive into building our Dioxus frontend, where I tackle real-time data visualization, UI performance, and making everything feel smooth for users🚀"
      //   }
      //   p {
      //     "Happy coding and keep building cool things✌"
      //   }
      // }
      section { 
        h2 { "Deploying the engine: Cloud, Containers & Data Streaming" }
        p {
          "At this point, all that was left was to package our order-matching engine and get it running in the wild—aka hosting it on the cloud. A typical cloud setup involves compute and storage providers like AWS, GCP, or if you’re feeling adventurous, self-hosted bare metal (good luck with that!)."
        }
        p {
          "For this project, we went the "
          i {"sane"} 
          " route: dockerizing our Axum server and deploying it on GCP (Google Cloud Platform). I was again pleasantly surprised by the ridiculously small Rust image size—85.7MB! This was a breath of fresh air considering my past experience with bloated Docker images that stubbornly stayed in the 500-750MB range even after multistage builds. The magic here is Rust compiles down to a single binary, meaning no runtime dependencies, no bulky OS libraries—just a neat little .exe (or its Linux/MacOs equivalent) that you can toss onto a server and run like a charm. Minimal overhead, maximum speed."
        }
        h3 { "Keeping Things Modular: Future Enhancements" }
        p {
          "To keep things manageable, I decided to park a couple of key upgrades as future add-ons:"
        }
        ol { 
          li { 
            strong { "Upgrading Axum to a Full-Fledged Web Server" }
            p { "Right now, our Axum server is a fast and reliable trading-machine, but it’s still pretty "
            i { "barebones" }
            " when it comes to load balancing and high-performance optimizations. Enter Pingora—Cloudflare’s battle-tested Rust framework for high-performance web serving. (TODO...)  Pingora is part of the larger push towards memory-safe alternatives to C, something even the US government has been advocating for. If you’re curious, check out Cloudflare’s docs on Pingora—it’s a great read on why Rust has been taking over the systems programming world." }
           },
          li { 
            strong { "Data Streaming: Because Throwing Away Data is a Crime" }
            p { "Right now, at the end of our simulation, order book updates just vanish into the void—no persistence, no historical state management. That’s unacceptable, and my data engineering instincts were already screaming at me to set up "
            em { "Kafka" }
            ", or at least "
            em { "rust-rdKafka" }
            "—the Rust interface to "
            em { "librdkafka" }
            " which in turn 
            is a C implementation of the "
            em{ "Apache Kafka" }
            " protocol, which itself is written in Java and Scala. That's a lot of abstractions, which doesn't seem right!" },
            p { "So, in my quest to find a pure Rust alternative, two projects caught my eye:" },  
            ul { 
              li { 
                strong { "Fluvio: " },
                "A lean distributed data streaming engine written in Rust. Think of it as a "
                i { "Apache Flink"}
                " + "
                i{ "Kafka"}
                " hybrid but optimized for real-time streaming workloads. Fluvio supports connectors for Rust, Python, and Go, letting you hook up your databases in AWS, Snowflake, and more. Plus, its Dataflow Engine allows traditional ETL transformations—a huge plus."
              },
              li { 
                strong { "Iggy: " },
                "A more recent project, but I’m always open to being amazed. Plus, Rust project names are just inherently cooler than their Java counterparts."
              },
            }
          }
        }
      }
      section { 
        h2 { "Next Stop: The Frontend" }
        p { 
          "With all the DevOps and data engineering battles fought (for now), it’s finally time to make things look pretty. Up next: building our real-time trading UI with Dioxus. Because what's the point of a blazing-fast order book if it isn’t wrapped in a sleek, intuitive interface?🚀"
        }
      }
    }
  }
}