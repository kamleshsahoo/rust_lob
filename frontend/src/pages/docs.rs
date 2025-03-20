use dioxus::prelude::*;
use crate::Route;

static BST_GIF: Asset = asset!("/assets/bst.gif");
static AVL_GIF: Asset = asset!("/assets/avl.gif");
static CSS: Asset = asset!("assets/docs.css");

#[component]
pub fn EngineDocs() -> Element {

  rsx! {
    document::Stylesheet {href: CSS},
    div {
      class: "docs",
      div {
        class: "docs-header", 
        h1 { 
          "Building a Real-Time Order Book - Part 1: The Engine",
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
          "When I set out to build a real-time limit orderbook application, I knew I needed a language that could handle the demands of high-frequency trading, real-time data processing, and seamless integration across the stack. After evaluating several options, I chose Rust—and for good reason. Rust is not just fast; it’s blazingly fast. But speed alone isn’t what makes " strong {"Rust"} " special. Its unique combination of performance, memory safety, and modern tooling makes it an ideal choice for building robust and lean high-performance systems."
        }
        p {
          "While this first part of the series focuses on building the orderbook engine, it’s worth noting that Rust’s versatility extends beyond just building the engine. In the subsequent parts of this series, we’ll explore how Rust powers the " em {"backend server"}" to handle real-time updates and file uploads, and how it enables us to build a " em {"dynamic frontend"} " using " strong {"WebAssembly"}". This approach ensures that our entire stack benefits from Rust’s performance and safety guarantees, while still writing everything in a single language. If you’re curious to learn more, check out the official " a {
          href: "https://doc.rust-lang.org/stable/book/", target: "_blank", "Rust book" }". Let's get started! 🚀"  
        }
      }
      section {
        h2 { "The Heart of the System: The Matching Engine" }
        p {
          "At the core of any trading system is the "
          strong { "matching engine" }
          "—the unsung hero quietly facilitating millions of transactions behind the scenes. Think of it as a diligent acccountant keeping track of buy and sell orders, constantly updating a ledger based on market events like new orders, modifications, or deletions."
        },
        h3 {"Order Book Basics"}
        p {
          "Before diving into the nitty-gritty implementation details, let's establish some terminology. In trading parlance, a " strong {"bid"} " is an order to buy an asset, while an " strong{"ask"} " is an order to sell. For example, in a stock exchange, the asset could be an Apple (AAPL) share, or on a crypto exchange like Kraken, it could be a currency pair like BTC-USD."
        }
        p {
          "The beauty of an order book lies in its simple yet powerful rules for execution. Orders are matched based on two fundamental principles:",
          ul {
            li { 
              strong { "Price Priority:" }
              " The best bid (highest buying price) or best ask (lowest selling price) gets priority."
            },
            li { 
              strong { "Time Priority (FIFO):" }
              " If two orders have the same price, the one that arrived first gets executed first."
            },
          }
        },
        p {
          "For this project, we have followed these core principles—respecting "
          em {"Price Priority"}
          " and using "
          em {"FIFO"}
          " for tie-breaking."
        }
        h3 { "Order Types" }
        p { "There are three fundamental order types that form the foundation of our system:" }
        ol {
          li { 
            strong { "ADD:" } " A new order is introduced to the book with a specific price and quantity."
          },
          li { 
            strong { "MODIFY:" } " An existing order's parameters are updated."
          },
          li { 
            strong { "CANCEL:" } " An existing order is removed from the book."
          },
        }
        p {
          "Every other order type is essentially a specialized version of these three primitives, differing only in the conditions that trigger a trade. For instance, a " em {"LIMIT"} " buy order specifies a condition like 'buy 100 shares of AAPL only if the price is $10.50 or lower.' Similarly, " i {"MARKET"} " orders execute instantly at current market prices, prioritizing immediate execution over price specificity. For this project, I focused on "
          em {"LIMIT"} " orders, as they provide a solid foundation for implementing other types."
        }
      },
      section {
        h2 { "Data Structures and architecture" }
        p { 
          "To build a real-time order book capable of handling high-frequency trading environments, I needed a data structure that could efficiently process at least 1 million transactions per second (TPS)."
        }
        p { 
          "Simple binary search trees or " i { "BST" } " were a natural starting point as they provide efficient lookups, but their performance can degrade to " i { "O(n)" } " in worst case scenarios when the tree becomes unbalanced—imagine a tree that looks more like a linked list. The animation below shows how a binary tree can grow in an uncontrolled manner."
        }
        p {
          "A better alternative is AVL tree, a type of self-balancing BST that maintains logarithmic time complexity "
          i { "O(log n)" } " by ensuring the tree remains balanced through automatic rotations. This makes insertions, deletions, and lookups much faster and more predictable, which is crucial for a trading system where every microsecond counts."
        }
        div {
          class: "gif-container",
          figure {
            class: "gif-item",
            img { 
              src: BST_GIF,
              alt: "Binary Tree Animation"
            }
            figcaption { "Binary Trees" }
          },
          figure { class: "gif-item",
            img { 
              src: AVL_GIF,
              alt: "AVL Tree Animation"
            }
            figcaption { "AVL Trees" }
          }
        }
        p {
          "AVL trees achieve this balance by storing a balance factor for each node, which is the height difference between its left and right subtrees. When this difference exceeds 1 (or falls below -1), the tree performs rotations to restore balance. These rotations are constant-time operations, making them efficient even for large datasets."
        }
        h3 { "Implementing an AVL Tree in Rust" }
        p {
          "Now comes the interesting part. Implementing an AVL tree in most languages is straightforward, but Rust—with its strict memory safety guarantees—presents unique challenges. Unlike traditional implementations that happily use raw pointers and risk memory leaks or dangling references, Rust demands safe memory management, making recursive tree data structures particularly tricky."
        } 
        p {"While Rust provides " 
          a {
            href: "https://doc.rust-lang.org/book/ch15-06-reference-cycles.html",
            target: "_blank",
            "smart pointers "
          }
          "such as " 
          code { "Rc<T>" }
          " and "
          code { "RefCell<T>" }
          " to handle recursive data structures, I quickly discovered these approaches led to increased code complexity. 'The fight with the borrow checker' was real and it was testing my patience."
        }
        p {
          "To simplify this and maintain my sanity, I opted for an " em { "Arena" } " based tree structure, where all nodes live in a collection (like a " code {"HashMap"} ") indexed by their prices. Instead of traditional parent-child pointers that create ownership cycles, each node carries an index to its position in the map, making traversal simple while avoiding multiple mutable borrows."
          pre { 
            class: "rust-code",
            code {
              span {
                class: "keyword",
                "struct"
              }
              " LimitNode {{
    price: " span {class:"keyword", "u64"}
    "
    volume: " span {class:"keyword", "u64"}
    "
    parent_idx: Option<" span {class:"keyword", "u64"} ">"
    "
    left_child_idx: Option<" span {class:"keyword", "u64"} ">"
    "
    right_child_idx: Option<" span {class:"keyword", "u64"} ">"
    "
    height: " span {class:"keyword", "i32"}
    span {class: "comment",
    "
    // other order metadata
"}"}}"
            }
            code {
              span {
                class: "keyword",
                "

struct"
              } " Arena {{"
    "
    buy_limits: HashMap<" span {class:"keyword", "u64, LimitNode"} ">"
    "
    sell_limits: HashMap<" span {class:"keyword", "u64, LimitNode"} ">"
    span {class: "comment",
    "
    // other book metadata
"}"}}"
            }
          }
        }
        
        p {"The arena-based approach eliminated the need for "
        i { "unsafe" } 
        " code and made the implementation significantly cleaner. The technique isn't something specific to rust but rather a clever data access pattern widely used in systems programming. This "
        a { 
          href: "https://rust-leipzig.github.io/architecture/2016/12/20/idiomatic-trees-in-rust/",
          target: "_blank",
          "blog"
        }    
        " is a good read for someone trying to implement tree and graph like data structures in rust, which explains how arena-based structures can be a life-saver."}
        p {
          "The only trade-off is that every tree operation must maintain an access to this " 
          i { "arena" }". However, this is a small price to pay for a cleaner, safer, and more ergonomic implementation."
        }
      },
      section { 
        h2 { "Engine Outputs" }
        p {
          "The engine maintains two separate AVL trees—one for the buy side (bids) and one for the sell side (asks). Beyond just matching orders, we capture several important outputs that give us insight into performance and market activity:"
        }
        h3 { "1. Order Processing Metrics" }
        p {"For each order processed by the engine, we record:"}
        ul {
          li { strong { "Execution latency" } ": Measures the time elapsed from when an order enters the engine until it exits, recorded in nanoseconds."}
          li { strong { "Number of executed trades" } ": Tracks how many trades were triggered by a single order." }
          li { strong { "Number of AVL rebalances" } ": Counts the number of tree rotations needed to maintain balance, giving us insight into algorithmic efficiency." }
          li { strong { "Order type" } ": Identifies whether the order was an ADD, MODIFY, or CANCEL type." }
        }
        p {
          "These metrics feed into our performance visualizations, helping us identify bottlenecks and optimization opportunities."
        }
        h3 { "2. Live Orderbook Snapshots" }
        p {
          "We collect the top " code { "n" } " bids and asks to update our real-time orderbook. This is implemented using in-order tree traversal with a small twist:"
        }
        ul { 
          li {"For bids (buy orders), we want them in descending order (highest price first), so we visit the right subtree first."}
          li {"For asks (sell orders), we want them in ascending order (lowest price first), so we visit the left subtree first."}
        }
        h3 {"3. Executed Trade Records"}
        p {"Every executed trade logs:"}
        ul { 
          li { strong { "Aggressive ID (AId)" } ": The order that triggered the trade" }
          li { strong { "Passive ID (PId)" } ": The existing order that got matched" }
          li { strong { "Price" } ": The execution price, which is always the passive order's price" }
          li { strong { "Volume" } ": The quantity traded, which is the minimum of both order volumes if they differ" }
        }
        p {"Example:"}
        pre {
          style: "background: none", 
          code {
            style: "background: none; color: var(--text-muted);", 
            span {
"     BidSide               AskSide"}
            span { "
Id, Price, Volume     Id, Price, Volume
1    9.2     15       11   9.5    100
2    8.5     20       22   10.5    10
"}
          }
        }
        p {
          "Currently, no trades are executed since the " strong { "best bid (9.2) < best ask (9.5)" }". But if a new order " strong {"(Id: 3, Buy@10, 5 units)"}" arrives, a trade is executed since the best bid price (now 10) is > the best ask price (9.5). The engine records this trade as:"
        }
        pre {
          style: "background: none", 
          code {
            style: "background: none; color: var(--text-muted);", 
            span { "AggressiveId(AId),  PassiveId(PId), Price,  Volume
    3                 11              9.5      5
"}
          }
        }
        p {"These three output types form the foundation of our application's data flow:"}
        ul {
          li {"Performance metrics feed into visualizations and graphs"}
          li {"Price level information updates the live orderbook table (showing top 20 levels)"}
          li {"Trade records populate the trades table (showing the 25 most recent executions)"}
        }
      }
      section {
        h2 { "Operational Modes" }
        p { "Since we have a structured input format, we built two ways to run the engine:" }
        h3 {"1. Simulation Mode"}
        p {
          "In simulation mode, we generate synthetic orders from a normal distribution with configurable mean and standard deviation parameters. The orderbook is pre-seeded with 10,000 initial orders—bids below the mean and asks above it—creating a realistic starting point similar to how real-world orderbooks carry over from previous trading sessions."
        }
        p {
          "As the simulation progresses, the center of the book shifts according to the chosen parameters, creating realistic price movement patterns. For modifications and cancellations, we select random existing order IDs from the book, ensuring we always have legitimate targets. If the order count falls below our 10,000 threshold, we automatically fall back to generating ADD orders to maintain sufficient market depth."
        }
        p {
          "This explains why you might see ADD orders appearing even when you've set their probability to zero in the simulation parameters—it's a failsafe mechanism to ensure the book doesn't become depleted."
        }
        h3 {"2. File Upload Mode"}
        p {
          "For backtesting against historical data or running predetermined scenarios, the file upload mode accepts order data from external sources. In this mode, we skip the pre-seeding phase and directly process orders as they appear in the file."
        }
        p {
          "The details of file parsing, validation, and error handling fall outside the scope of the core engine and will be covered in our upcoming backend documentation."
        }
      }
      section { 
        h2 { "What’s Next?" }
        p {
          "With our high-performance orderbook engine in place, we've established the foundation of our trading application. It efficiently processes orders, maintains price-time priority, and provides rich output data for analysis and visualization."
        }
        p {
          "So, it's time to connect it to the real world. A trading engine that executes orders but doesn’t tell anyone about it? Not very useful. In the next part of this series, we'll explore the backend system that brings this engine to life, providing APIs, data persistence, and integration points for our frontend interface."
        }
        p {
          "Stay tuned—the journey from trading engine to full-fledged trading platform is just getting started💫"
        }
      }
    }
  }
}


#[component]
pub fn BackDocs() -> Element {
  rsx! {
    document::Stylesheet {href: CSS},
    div { 
      class: "docs",
      div { 
        class: "docs-header",
        h1 { 
          "Building a Real-Time Order Book - Part 2: The Backend"
        }
        p {
          "Welcome back to the second part of our journey building a real-time orderbook application. If you missed the " Link{to: Route::EngineDocs {  }, "first part"} " where we implemented our high-performance matching engine, I highly recommend checking it out before diving in here."
        }
        p {
          "With our efficient order-matching engine in place, the next challenge was figuring out how to deliver real-time updates to users. Rust proved to be the perfect choice here as well, providing all the ingredients needed to build a professional-grade server. The Rust ecosystem seamlessly supports complex socket programming protocols like WebSockets, asynchronous tasks, and middleware features like rate limiting and authorization. In this post, we’ll dive into how we built the backend to serve our orderbook engine, ensuring it’s secure, scalable, and ready for the cloud."
        }
      }
      section {
        h2 {"The Game Plan"}
        p {
          "To put things in perspective, let’s outline what we’re aiming to achieve with our backend:"
        }
        ol {
          li { strong{"Build API Endpoints"}": Transmit the orderbook engine’s outputs to users in real-time."},
          li { strong{"Implement Authorization and Rate Limiting"}": Keep track of orders and prevent unauthorized access."},
          li { strong{"Deploy to the Cloud"}": To power a user-facing web page."},
        }
      }
      section {
        h2 {"Server and API Endpoints"}
        p {
          "A server is essentially a sophisticated wrapper around core application logic that (as its name suggests) needs to be " i {"served"} " to the outside world. By " i {"outside world"} ", I mean users trying to access our orderbook engine over the internet via a web page. In network programming jargon, the user is the " i {"client"}" and the server is, well, the " i{"server"} ". We'll stick to this terminology for clarity."
        }
        p {
          "In our case, the application logic is the orderbook engine we built in the previous part. Building a wrapper layer doesn't seem like a big deal, right? Well, sending data over the internet is far from straightforward in today's landscape of security concerns and data breaches—all while maintaining high efficiency so users don't have a poor experience."
        }
        h3 {"HTTP and TCP: The Backbone of Data Transfer"}
        p {
          "Data transfer over the internet primarily happens using " strong {"HTTP protocols"} ", which dictate how requests and responses are structured. Common HTTP methods include " em {"GET"}", " em {"PUT"}", " em {"POST"} " etc. Underlying these HTTP methods is the " strong {"TCP protocol"} ", which ensures a reliable connection between the client and server. TCP handles the nitty-gritty details, like establishing a connection (via the famous " em {"3-way handshake"}") and ensuring data reaches its destination."
        }
        p {
          "Thankfully, these complexities are abstracted away when you make a simple " code {"GET"}" request. However, it’s the server’s job to handle all this under the hood."
        }
        h3 {"Handling Multiple Clients: Asynchronous Paradigms"}
        p {
          "One of the key challenges in server design is handling multiple clients simultaneously. This is where asynchronous programming comes into play. By leveraging Rust’s async capabilities, we can handle multiple users without blocking execution. This is crucial for maintaining a smooth user experience, especially in a real-time application like ours."
        }
        h3 {"Middleware"}
        p {
          "Middleware is additional logic that sits between the client request and the core application. It handles tasks like authorization, security, and rate limiting. For example, if a user exceeds the allowed number of requests, middleware can reject further requests to prevent server overload."
        }
        p {
          "These concerns make server implementations both interesting and a great learning experience, as we'll see."
        }
      }
      section {
        h2 {"Choosing Axum for the Backend"}
        p {
          "We'll be using Axum to build our server for two compelling reasons:"
          ol {
            li {strong{"Tokio Ecosystem"} ": Axum is built on top of Tokio, Rust’s premier asynchronous runtime. This allows us to write highly efficient network applications."},
            li {strong{"Modular Middleware"} ": Axum doesn’t have its own middleware system. Instead, it uses " em{"Tower"} ", a modular library for building middleware layers. This lets us stack custom middleware like rate limiters, load balancers, and more, making our server highly scalable."}
          }
        }
        p {
          "While " strong {"Actix"} " is another popular Rust framework worth exploring, Axum’s simplicity and integration with Tokio made it the perfect choice for our needs."
        }
      }
      section {
        h2 {"Designing API Endpoints"}
        p {
          "As discussed in the engine documentation, we offer two modes for the orderbook engine, which dictate how we structure our API endpoints:"
        }
        h3 {"1. WebSockets: Real-Time Streaming for Live Dashboards"}
        p {
          "Since our real-time dashboard needs continuous updates, WebSockets are the ideal choice. Unlike traditional HTTP requests (GET, POST, etc.), which require repeated polling, WebSockets establish a persistent connection, enabling seamless data flow between the server and clients."
        }
        p {
          "WebSockets start as HTTP requests before upgrading to a WebSocket connection via an " code {"Upgrade"}" header. This handshake phase also allows us to pass additional parameters, which we'll leverage when adding authentication middleware."
        }
        p {"Here's how our WebSocket implementation sends updates:"}
        ul {
          li {"The orderbook engine runs as an async task producing data updates"},
          li {"These updates are sent via a WebSocket stream using " em {"Tokio channels"}" for thread-safe message passing."},
          li {"The main thread remains free for handling connections, maximizing performance."},
        }
        p {"The main components of socket programming, looks like so:"}
        pre {
          class: "rust-code",
          code {
            span {
              class: "keyword",
              "async fn"
            }
            " handle_socket(
    socket: "
            span { class: "keyword", "WebSocket" }
            span { class: "comment", 
    "
    // other extractors"
            }
      
      "
  ) {{"     span { class: "keyword", "
    let" } " (" span { class: "keyword", "mut" } " sender, " span {class: "keyword", "mut"} " receiver) = socket.split();" span { class: "comment", "
    // channel to send updates from the orderbook engine to the main thread" }
            span { class: "keyword", "
    let" } " (tx, " span { class: "keyword", "mut" } " rx) = mpsc::channel(10_000);"
    span { class: "keyword", "
    loop"} " {{" "
      tokio::" span { class: "keyword", "select!"} " {{" span { class: "comment", "
        // receive messages from client over websocket"} "
        msg = receiver.next() => {{" span { class: "keyword", "
          if let Some"} "(" span { class: "keyword", "Ok"} "(client_msg)) = msg {{
            " span { class: "comment", "// parse client message & spawn a task to process the message" } "
            tokio::" span { class: "keyword", "spawn" } "(order_book_engine(tx.clone(), client_params));" span { class: "comment", "
            // rem. logic to break out early, for eg. when a rate-limit is breached" }
            "
            }}
        }}," span { class: "comment", "
        // receive orderbook updates on the channel"} span { class: "keyword", "
        Some" } "(data_update) = rx.recv() => {{" span { class: "comment", "
          // send the updates to client over websocket" } "
          sender.send(data_update)." span { class: "keyword", "await" } ";" span { class: "comment", "
          // other serverside logic" } "
        }}
      }}
    }} " span { class: "comment", "
    // websocket connection is droped here and the connection is closed" }"
  }}"  
          }
        }
        p {
          "This architecture ensures trades get executed while updates are instantly streamed to users, all without blocking execution. Fun fact: I had to throttle the server updates to prevent the UI from crashing—a humbling moment for any backend engineer. If you're interested in real-time systems or chat applications, the Tokio documentation is a goldmine of async wizardry."
        }
        h3 {"2. HTTP: File Uploads for Bulk Processing"}
        p {"For the file-upload mode, we need to process large order datasets and return results in one go. This required efficiently sending file data to the server."}
        p {
          "We implemented a simple POST endpoint that accepts requests with file contents in the body. Initially, we parsed files on the client side and sent the parsed orders directly. This approach hit a bottleneck with larger files, prompting us to chunk and compress the parsed orders."
        }
        p {
          "Client-side parsing worked well even with large files, except that the website became unresponsive during parsing. Investigation revealed that despite leveraging Rust's speed (via " em {"WebAssembly"} ") to parse millions of orders, the parsing task blocked the main UI thread. While workarounds like Web Workers exist, we prioritized simplicity."
        }
        p {
          "The natural progression was moving parsing logic server-side for larger files. Sending large files over HTTP connections can overwhelm the underlying TCP connection. We considered several solutions:"
        }
        ul {
          li {"Chunk and compress file contents (as we did for smaller files)"},
          li {"Use HTTP/2 protocol for more efficient file transfers"},
          li {"Use cloud storage to upload files and have the server access them from there"},
        }
        p {
          "While HTTP/2 worked in initial tests, we settled on the first option for consistency. The third option is worth exploring but requires some finnicky cloud configurations we decided to skip for now."
        }
      }
      section { 
        h2 { "Middlewares: Security and Scalability" }
        p {
          "Our server now has endpoints for communicating with web pages, but it remains basic in terms of security, especially with our " em {"no-login"} " access model. We also need limits on server traffic capacity."
        }
        p {
          "These additional logic layers between our core engine and incoming client requests are aptly called "em {"middleware"}". If you've ever seen a message like " code{"You've reached our limit of 80 messages per hour. Please try again later"}" while using ChatGPT or Claude, that's middleware in action."
        }
        h3 { "1. Authorization: Request Signing" }
        p { "Adding middleware resembles wrapping our server endpoints in an onion. When our server receives a client request, it passes through these layers like so:"}
        pre { 
          code {
        "                  requests
                      |
                      v
     +-----------------------------------+              
     |            cors_layer             |
     +-----------------------------------+
     |        authorization_layer        |
     |     +------------------------+    |
     |     |      rate_limiter      |    |
     |     |    +--------------+    |    |
     |     |    |  /endpoints  |    |    |
     |     |    +--------------+    |    |
     |     |     rate_limiter       |    |
     |     +------------------------+    |
     |        authorization_layer        |
     +-----------------------------------+
     |            cors_layer             |
     +-----------------------------------+
                      |
                      v
                  responses"
          }
        }
        p { "The CORS layer does some basic checks like verifying the allowed headers, HTTP methods etc. and passes the request to the next layer, if all checks are passed. The authorization layer checks if the client is authorized to make the request. If yes, it proceeds; if no, it rejects with an error message. The rate limiter checks if the client has exceeded their order limit within a given time window, rejecting or proceeding accordingly. As the Axum docs note: " em {"Although this is more complicated in practice, it's a good mental model to have."}
        }
        p {
          "For authorization, we implemented a simple yet effective request signing method:"
        }
        ul {
          li {"With each client request, we send the current timestamp and a cryptographic signature (a " strong{"HMAC-SHA256"}" hash of concatenated request details) generated using a secret key"},
          li {"When the server receives a request, the authorization middleware extracts these headers, rejecting requests without them"},
          li {"If headers are found, we first check if the timestamp is recent enough, then verify the signature using our secret key"},
        }
        p { "This provides robust authentication while maintaining our 'no-login' approach." }
        h3 { "2. Rate Limiting: Preventing Overload" }
        p { "Once authorization checks pass, the request moves to the rate-limiting layer, where we check the number of orders a user has sent within a certain time window." }
        p {"For this, I applied some data engineering techniques:"}
        ul {
          li { "Spun up a small Redis database instance to maintain two counters: one at the user IP address level and another at the global application level" }
          li { "When a new user arrives, we create a key with an expiration timeout and update the counter with their order count" }
          li {"If a user exceeds a certain threshold (e.g., 2 million orders in a time window), we reject the request with a rate limit message"}
          li {"When the expiration time passes, the key gets deleted and the user can resume sending orders"}
          li {"The global rate limit works in an identical fashion, except the fact that the counter is incremented for every incoming request, which helps to achieve a system-wide rate limiting effect"}
        }
        p {
          "This is a classic use case for Redis as an in-memory database—extremely fast compared to traditional SQL or PostgreSQL—making rate limit checks virtually instantaneous. We do use Postgres for logging engine statistics like total processed orders."
        }
      }
      section {
        h2 { "The Cloud and DevOps" }
        p {
          "Initially, our orderbook engine ran on my personal laptop—effectively making it the server with internal RAM as memory and hard disk as storage. As long as my laptop had uninterrupted power and internet, it could serve the orderbook engine. This " em {"bare metal"} " setup meant manually managing uptime, software updates, and memory usage as user numbers grew."
        }
        p {
          "This is where cloud providers like GCP, AWS, and Azure come in. They let you use virtual machines with configurable RAM and memory, without the physical hardware. For this project, we used GCP, though the server setup would be similar across providers."
        }
        p {
          "GCP calls these provisionable " em {"laptops"}", compute engines and we specifically used GCP Cloud Run. Cloud Run eliminates the hassle of managing compute resources as user numbers fluctuate. This "scaling" automatically adjusts resources based on usage—similar to needing more RAM when playing the latest " i{"Elden Ring"} " title."
        }
        h3 { "Dockerizing the Serve" }
        p { "To deploy our Axum server, we packaged it into a Docker container. Thanks to Rust’s minimal runtime dependencies, the resulting image was a mere 85MB—a far cry from the bloated 500MB+ images I’ve dealt with in the past. Rust compiles down to a single binary, meaning no runtime dependencies or bulky OS libraries. Just a lean, mean, serving machine." }
      }
      section { 
        h2 { "Next Stop: The Frontend" }
        p { 
          "With all the DevOps and data engineering battles fought (for now), it’s finally time to make things look pretty. Up next: building our real-time trading UI with Dioxus—because what's the point of a blazing-fast order book if it isn’t wrapped in a sleek, intuitive interface?🚀"
        }
      }
    }
  }
}

#[component]
pub fn FrontDocs() -> Element {

  rsx!{
    document::Stylesheet {href: CSS},
    div {
      class: "docs", 
      div {
        class: "docs-header", 
        h1 { 
          "Building a Real-Time Order Book - Part 3: The Frontend",
        }
        p {
          "Welcome back, fellow coders! This is the third (and final) installment of our journey to build a real-time limit orderbook application. If you missed the first two parts, where we built the orderbook engine and set up our backend server, I highly recommend you check them out. But don’t worry, I’ll give you a quick recap."
        }
      },
      section { 
        h2 { "Recap: The Engine & Server" }
        p {
          "In the previous two parts, we developed a high-performance limit order book engine, exposing it via a WebSocket for real-time data updates and POST endpoints for order submissions. We also added some middleware to our server enforcing rate-limiting and security measures. Now, it’s time to focus on the frontend—the part of the application that users will interact with."
        }
      }
      section {
        h2 { "Building Blocks of the Web" } 
        p { 
          "At its core, every website we see today is just a glorified, well-dressed HTML page. "
          strong{"HTML"} 
          " ("
          em {"HyperText Markup Language"}
          ") is the backbone that defines the content and structure of a website—think of it as the skeleton. But a skeleton alone isn’t much to look at. That’s where "
          strong{"CSS"}
          " ("
          i {"Cascading Style Sheets"}
          ") comes in, giving our web pages the much-needed—colors, layouts, animations, and all the fancy design elements. And then the final piece—"
          strong{"JavaScript"}
          ", which adds interactivity, making our pages dynamic, like fetching your order total when you click that "
          em{"Checkout"}
          " button (or reminding you that you probably don’t need that 10th GPU in your cart)."
        }
        p {
          "If you're curious about how web standards evolved and who ensures they don’t spiral into chaos, you can read about the HTML living standard "
          a {
            href: "https://html.spec.whatwg.org/multipage/introduction.html#is-this-html5?",
            target: "_blank",
            "here"
          }
          " maintained by "
          strong{"WHATWG"}
          "—an association of major browser vendors (Apple, Google, Mozilla, and Microsoft). But if you’re just looking to start building without diving into the history books, "
          a {
            href: "https://developer.mozilla.org/en-US/docs/Learn_web_development/Core/Structuring_content",
            target: "_blank",
            "MDN Docs"
          }
          " are your best friend—it’s packed with everything you need to get started with minimal programming knowledge."
        }
        p {
          "Now, modern websites aren’t just built with raw "
          i {"HTML"}
          ", "
          i {"CSS"}
          ", and "
          i {"JavaScript"}
          ". We have a flood of frameworks and libraries that make development easier and more powerful. For styling, we have "
          strong {"Tailwind CSS"}
          ", "
          strong {"Bootstrap"}
          ", and many others. For interactivity and state management, we have JavaScript frameworks like "
          i {"React"}
          ", " 
          i {"Vue"}
          ", and "
          i {"Svelte"}
          " that let us build complex, scalable applications with relative ease, but they all boil down to the three core web technologies we talked at the start. With all that in mind, it was time to choose the right tools for my orderbook web app. Since I had already built my backend in Rust, wouldn't it be awesome if I could write my frontend in Rust too? Spoiler alert: "
          strong {"Yes, it would!💫"}
        }
      }
      section {
        h2 { "Dioxus: Supercharging Frontend with Rust" },
        p { "Given that my backend was already in Rust, the thought of maintaining a separate JavaScript-based frontend didn’t exactly light up my day—why let go of Rust’s speed and the sheer joy of writing it? "
        strong { "Dioxus" }
        ", does exactly that and more! Dioxus, lets you write your frontend entirely in Rust (okay, maybe with a sprinkle of JavaScript here and there)." }

        p {
          "But Dioxus isn’t just about convenience; it’s about power. "
          em {"WebAssembly"}
          " or " 
          em {"WASM"}
          " frameworks like Dioxus enable us to build scalable, performant, and production-ready web applications—all in Rust. And here's the best part—"
          strong {"Dioxus lets you build and ship applications for web, desktop, and mobile platforms using the same codebase!"} 
          " How cool is that? If that made you curious, check out the "
          a {
            href: "https://dioxuslabs.com/",
            target: "_blank",
            "Dioxus docs"
          }
          "."
        }
        p {
          "Other Rust-based web and UI frameworks like "
          i {"Leptos"}
          ", "
          i {"Yew"}
          ", and "
          i {"Iced"}
          " are also gaining traction, each with its own strengths. The Rust frontend ecosystem may still be evolving, but it's already proving to be a serious contender in the world of web development."
        }
        p {
          "And with that, my tech stack decisions were set—Rust on the backend, Rust on the frontend, and a whole lot of fun ahead!🚀"
        }
      }
      section { 
        h2 { "Page Components and Reactivity" }
        p {
          "Once the Dioxus setup was complete (you can find the step-by-step guide "
          a {
            href: "https://dioxuslabs.com/learn/0.6/getting_started/#",
            target: "_blank",
            "here"
          }
          "), it was time to actually build something. Coming from "
          i {"React"}
          " and "
          i {"Next.js"}
          ", I was used to "
          em {"JSX/TSX"}
          " extensions, along with the dynamic duo of "
          code {"useState"}
          " and "
          code {"useEffect"}
          ". "
          em {"JSX"}
          " is a syntax extension that lets you write HTML inside your JavaScript code, while "
          code {"useState"}
          " helps manage component state—like showing an order total when a user clicks "
          i {"Checkout"}
          "."
        },
        p {
          "Other frameworks take different approaches to state management. "
          em {"Vue"}
          " has "
          code {"ref()"}
          ", " 
          em {"Solid.js"}
          " has "
          code {"createSignal"}
          ", and "
          em {"Svelte"}
          " keeps things simple with "
          code {"$state"}
          ". Dioxus, combines the best of all these frameworks into its ergonomic state management system while keeping things familiar for React devs."
        }
        p {
          "For example, in "
          em { "React" },
          ", I will write a header component like this: "
          pre { 
            class: "js-code",
            code {
              span { 
                class: "comment",
                "// React component"
              }
              span { 
                class: "keyword",
"
 export default function"
              }
              " Header () {{ "
              span {
                class: "keyword",
    "
    return "
              }
              "("
              span { 
                class: "keyword",
                "<h1> "
              }
              span {
                class: "string",
                "My WebPage Header"
              },
              span { 
                class: "keyword",
                " </h1>"
              }
              ");
 }}"
            }
          }
        "In "
        em { "Dioxus" }
        ", the equivalent would be:"
        pre { 
          class: "rust-code",
          code {
            span { 
              class: "comment",
              "// Dioxus component"
            }
            span { 
              class: "keyword",
"
#[component]
pub fn"
            }
            " Header () -> "
            span { 
              class: "keyword",
              "Element"
            },
            " {{"
            span {
              class: "keyword",
  "
  rsx!"
            }
            " {{ ",
            span {
              class: "keyword",
              "h1"
            },
            " {{ "
            span {
              class: "string",
              r#""My WebPage Header""#
            }
            " }} }}",
          }
        }
          "See the similarities? Both require returning a single root element and follow the same general principles (you can dive deeper into the Dioxus docs for the nitty-gritty details)."
        }
        p {
          "State management in Dioxus is also similar to React. Instead of "
          code {"useState"}
          ", " 
          em { "Dioxus" }
          " uses " 
          code {"use_signal"}
          " like so:"
          pre { 
            class: "rust-code",
            code {
              span { 
                class: "comment",
                "// Dioxus states with use_signal()"
              }
              span { 
                class: "keyword",
                "
 let mut"
              }
              " user_clicked: "
              span { 
                class: "keyword",
                "Signal<bool>"
              },
              " = "
              span {
                class: "keyword",
                "use_signal"
              }
              "(|| ",
              span {
                class: "keyword",
                "false"
              },
              ");"
              span {
                class: "keyword",
                "

 fn"
              }
              " click_handler() {{
    user_clicked."
              span { 
                class: "keyword",
                "set"
              }
              "("
              span {
                class: "keyword",
                "true"
              }
              ");"
              span { 
                class: "comment",
                "
    // ..."
              }
              "
 }}"
            }
          }
          "In Dioxus, "
          code {"use_signal"}
          " is the equivalent of React’s "
          code {"useState"}
          ", keeping things reactive while embracing Rust’s functional paradigm."
        }
      }
      section {
        h2 { "Routing: Enum-Based Simplicity" }
        p { "Dioxus takes a different approach to routing—no file or folder-based routing madness here. Instead, routes are defined as an "
        code {"enum"}
        " where each variant represents a path in your app. That means one can "
        em {"nest routes"}
        ", " 
        em {"apply layouts"}
        ", and " 
        em {"handle dynamic paths"}
        " with pure Rust enums." }
        p {
          "Here’s how our app’s router is structured:"
          pre {
            class: "rust-code",
            code{
              span {
                class: "comment",
                "// Dioxus routing"
              }
              span { 
                class: "keyword",
                "
  #[derive"
              }
              "("
              span { 
                class: "keyword",
                "Routable"
              }
              ", "
              span { 
                class: "keyword",
                "PartialEq"
              }
              ", "
              span { 
                class: "keyword",
                "Clone"
              }
              ")"
              span {class: "keyword", "]"}
              span { 
                class: "keyword",
                "
  enum"
              }
              " Route {{"
              span { 
                class: "keyword",
                "
      #[layout"
              }
              "(Template)"
              span { 
                class: "keyword",
                "]
      #[route"
              }
              "("
              span { 
                class: "string",
                r#""/""#
              }
              ")"
              span {class: "keyword", "]"}
      "
      Home {{}},"
              span {
                class: "keyword",
                "
      #[nest"
              }
              "("
              span {
                class: "string",
                "/docs"
              }
              ")"
              span { 
                class: "keyword",
                "]
          #[route"
              }
              "("
              span { 
                class: "string",
                r#""/""#
              }
              ")"
              span {class: "keyword", "]"}
          "
          EngineDocs {{}},"
              span { 
                class: "keyword",
                "
          #[route"
              }
              "("
              span { 
                class: "string",
                r#""/backend""#
              }
              ")"
              span {class: "keyword", "]"}
          "
          BackDocs {{}},"
              span { 
                class: "keyword",
                "
          #[route"
              }
              "("
              span { 
                class: "string",
                r#""/frontend""#
              }
              ")"
              span {class: "keyword", "]"}
          "
          FrontDocs {{}},"
              span { 
                class: "keyword",
                "
      #[route"
              }
              "("
              span { 
                class: "string",
                r#""/simulator""#
              }
              ")"
              span {class: "keyword", "]"}
      "
      Simulator {{}},"
              span { 
                class: "keyword",
                "
      #[route"
              }
              "("
              span { 
                class: "string",
                r#""/:..route""#
              }
              ")"
              span {class: "keyword", "]"}
      "
      PageNotFound {{ route: "
              span {
                class: "keyword",
                "Vec<String>"
              }
              " }}
  }}"
            }
          }
        }
        p {
          "For React devs, this should feel somewhat familiar—it’s like "
          em {"react-router"} 
          " but with more type safety and control. For example, navigating to "
          code {"/docs/backend"}
          " serves our backend documentation, while "
          code {"/simulator"}
          " loads our trading simulator. "
          i {"No unnecessary boilerplate, just a clean and flexible routing system."}" This approach to routing is incredibly powerful. Instead of being locked into directory structures, we define routes programmatically, making it easy to scale, apply dynamic parameters, and nest layouts as needed."
        }
      }
      section {
        h2 { "Building the Simulator Page" }
        p {
          "The simulation dashboard is the heart of our frontend application. It allows users to interact with the orderbook engine we built in Part 1. This is where users can "
          i {"run simulations and interact with the order book engine"} 
          " in real time."
          br {}
          "The simulation dashboard has two main modes:"
        }
        h3 {"1. Simulation Mode"}
        p {
          "In this mode, users can run a simulated limit order book with "
          i {"synthetically generated orders"}
          ", customized using user-defined parameters. The results are streamed in real time and visualized through:"
          ul {
            li { strong {"A live order book table"} },
            li { strong {"Dynamic graphs"} },
            li { strong {"Real-time statistics"} },
          }
        }
        p {"Now, there are few things to consider here:"}
        h4 { "a. Handling WebSocket Updates Efficiently" }
        p {
          "We needed a way to process and display incoming server updates smoothly. This meant:"
          ul {
            li { strong {"Receiving real-time updates"} " via the WebSocket connection." }
            li { strong {"Displaying them seamlessly"} " without causing UI lag." }
          }
        }
        p {
          "To achieve this, we leveraged Dioxus' "
          em {"asynchronous hooks"}
          ", specifically "
          code {"spawn"}
          " and "
          code {"use_coroutine"}
          ". These allow us to handle asynchronous tasks, like maintaining a persistent WebSocket connection. Rust's "
          em {"async"}
          " model really shines here—one task continuously "
          i {"listens for updates"}
          " from the WebSocket, while a second task "
          i {"updates the UI state"}
          ", ensuring our graphs, order book table, and statistics reflect the latest data. The two tasks communicate via "
          strong {"channels"}
          ", similar to how our backend order generator interacts with the WebSocket sender."
        }
        h4 { "b. Throttling Updates for Performance" }
        p {
          "Our backend engine processes orders at "
          i { "millions of transactions per second" }
          ". While this is great, two issues arise:"
          ul {
            li { 
              "The WebSocket "
              strong {"can't handle that much traffic"}
              "—too many updates could cause connection drops." }
            li { 
              "Even if we could process every update, "
              strong {"humans can't perceive more than ~60 updates per second"}
              " (60 fps), which is a frame every "
              strong { "16 milliseconds" }
              "."
            }
          }
        }
        p {
          "So, we throttle incoming updates using a "
          em {"signal"}
          " that tracks whether an update is needed. For instance, our real-time "
          strong {"3D graphs"}
          " update at a set interval (e.g., every 50ms) to maintain performance "
          i {"without overwhelming the UI"}
          "."
        }
        h4 { "c. Choosing the Right Graphing Library" }
        p {
          "We intially experimented with  "
          a { 
            href: "https://crates.io/crates/plotters",
            target: "_blank",
            "Plotters"
          }
          ", but later moved to "
          em {"Apache ECharts"}
          " a web-based visualization library with Rust bindings via the "
          a {
            href: "https://crates.io/crates/charming",
            target: "_blank",
            "charming"
          }
          " crate. It provides more interactive and visually appealing graphs, making real-time data representation seamless. Regardless of the library, the approach remains the same—our "
          em {"HTML"}
          " "
          code {"canvas"}
          " element renders the graphs, and as long as we keep updating the data at regular intervals, we get fluid real-time visualizations."
        }
        h3 {"2. File Upload Mode"}
        p { "Unlike the simulation mode, this mode is designed for users to upload their own order book data and analyze it."}
        p {
          "Here’s how it works:"
          ul {
            li { 
              "Users upload a "
              i {".txt"}
              " file containing order data in a predefined format." },
            li { "The file is parsed and validated on the client side." },
            li { 
              "Orders not following the format are ignored, and a preview table displays the top and bottom 5 valid rows. (If the file has more than 10 rows, ellipsis rows are added to indicate more data exists.)"
            }
          }
        }
        h4 { "a. Handling Large Files Efficiently" }
        p {
          "There were two critical challenges here:"
          ol {
            li { 
              strong{"Client-side parsing performance"}
              ul { 
                li { "For files up to 5MB, parsing happens entirely in the browser, meaning files are parsed even if our backend server is down."},
                li { "Thanks to Dioxus' WASM runtime, this is blazing fast—I tested a 10-million-order file, and the parsed preview table loaded in about ~8 seconds🚀" }
              }
            }
            li {
              strong {"Uploading Large Order Books Efficiently"}
              ul {
                li { 
                  "Once parsed, the file is sent to the server via "
                  strong {"POST"}
                  " request"
                },
                li { "To prevent blocking the main UI thread, we chunk the parsed orders in batches of 10,000-order packets." },
                li { "The backend collects these chunks and feeds them into the order book engine once all packets are received." }
              }
            }
          }
        }
        p {
          "Our first naive approach worked, but at a cost. Since the entire processing happened on client side, the UI froze during uploads. Not great!"
        }
        h4 { "b. Fixing UI Freezes: Smart Upload Handling" }
        p {
          "To prevent UI crashes, we explored solutions like "
          em {"Web Workers"}
          ", but ultimately settled on a simpler fix:"
          ul { 
            li { 
              i {"Limit client-side file parsing and validation to 5MB files"}
              "."
            },
            li { 
              i {"Larger files are sent directly to the backend server"}
              ", avoiding client-side memory overload."
            }
          }
        }
        p {
          "This strikes a balance—users still get "
          strong {"instant validation"}
          " for small datasets, while larger datasets "
          strong {"offload processing"}
          " to the backend."
        }
      }
      section { 
        h2 { "Conclusion" }
        p {
          "And there we have it—a full-stack, real-time orderbook application built entirely in Rust! From the high-performance orderbook engine in Part 1, to the robust backend server in Part 2, and now the sleek, interactive frontend in Part 3, this journey has been a testament to Rust’s versatility and power across the entire stack."
        }
        h3 { "A Cohesive System" }
        p { 
          "The beauty of this project lies in how seamlessly each component integrates with the others. The backend, built with Axum and Tokio, efficiently handles real-time WebSocket connections and file uploads, ensuring that data flows smoothly between the orderbook engine and the frontend. The frontend, powered by Dioxus, leverages Rust’s WebAssembly capabilities to deliver a responsive, dynamic user interface. Together, they form a cohesive system that not only performs exceptionally well but also maintains a clean, maintainable codebase."
        }
        h3 { "Challenges and Triumphs" }
        p { 
          "Building this application wasn’t without its challenges. From optimizing WebSocket updates to handling large file uploads without freezing the UI, each hurdle pushed us to think creatively and leverage Rust’s strengths. For instance, throttling WebSocket updates to balance performance and usability was a humbling reminder of the importance of user experience in real-time systems. Similarly, the decision to offload large file processing to the backend highlighted the need for thoughtful trade-offs between client-side and server-side responsibilities."
        }
        h3 { "The Frontend: Where Rust Shines" }
        p {
          "The frontend, in particular, showcased Rust’s potential in a domain traditionally dominated by JavaScript. Dioxus’s ergonomic API, combined with Rust’s powerful async model, made it possible to build a highly interactive and performant UI. Features like real-time orderbook visualization and dynamic graphing were made possible by Rust’s ability to handle complex computations efficiently, even in a browser environment. The use of WebAssembly ensured that our frontend could keep up with the backend’s blazing speed, delivering a seamless user experience."
        }
        h3 {"Looking Ahead"}
        p {"To keep things manageable, I decided to park a couple of key upgrades as future add-ons:"}
        h4 {"1. Data Streaming: Because Throwing Away Data is a Crime"}
        p { "Currently, at simulation end, orderbook updates vanish—no persistence or historical state management. As a data person, this bothered me. My instinct was to set up Kafka, or at least rust-rdKafka (the Rust interface to librdkafka, a C implementation of the Apache Kafka protocol, itself written in Java and Scala). That's too many abstraction layers! "}
        p {
          "In my quest for a pure Rust alternative, two projects caught my eye:"
        }
        ul {
          li { strong { "Fluvio" } ": A lean distributed data streaming engine written in Rust—think Apache Flink + Kafka hybrid optimized for real-time streaming workloads. Fluvio supports connectors for Rust, Python, and Go, connecting to databases in AWS, Snowflake, and more. Its Dataflow Engine enables traditional ETL transformations—a huge plus."}
          li { strong {"Iggy"} ": A more recent project, but I'm always open to being amazed. Plus, Rust project names are inherently cooler than their Java counterparts." }
        }
        h4 {"2. Upgrading Axum for High-Performance Load Balancing"}
        p { "Our Axum server is optimized for speed, but we could further improve scalability by integrating " strong{"Pingora"}"—Cloudflare’s battle-tested Rust framework for high-performance web serving. For folks setting up proxies and load balancers, this the Rust counterpart of " em{"Nginx"}". Pingora represents the larger push toward memory-safe alternatives to C, something even the US government advocates. Cloudflare's documentation on Pingora offers fascinating insights into why Rust is dominating the systems programming world." }
        h3 { "Final Thoughts" }
        p {
          "Building this application has been an incredibly rewarding experience. It’s a testament to what’s possible when you combine the right tools with a passion for solving complex problems. Rust’s ecosystem, though still evolving, has proven to be a powerful ally in building modern, high-performance applications. Whether you’re a backend engineer, a frontend developer, or someone who just loves to build things, I hope this series has inspired you to explore Rust and its potential."
        }
        p {
          "Thank you for joining me on this journey. If you’re as excited about Rust as I am, I encourage you to dive into the Dioxus docs or explore the Tokio ecosystem for your next project. The future of full-stack development is bright, and Rust is leading the charge"
        }
        p { "Until next time, happy coding and keep building cool things✌" }
      }
    }
  }
}