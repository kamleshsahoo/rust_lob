# Low-latency Order Book Application
A high-performance, full-stack orderbook application written in pure Rust, delivering nano-second level orderbook updates to a lean and performant WebAssembly(*WASM*) frontend.
>_You can find the live demo and blog [here](https://kamleshsahoo.github.io/lob_deploy/)._

# Features
![demo](assets/lob_demo.gif)
- **Efficient Orderbook Engine**: Implements *binary AVL* trees from scratch in pure Rust. Utilizes zero-copy memory principles and idiomatic Rust patterns to achieve performance with memory safety.
- **Real-Time Streaming**: Leverages *Tokio*'s asynchronous runtime and *WebSockets* to stream orderbook updates with minimal latency. Uses *channels* for efficient communication between system components.
- **Redis Authentication**: Implements high-performance authentication using *Redis* for token validation and session management, optimized for low-latency operations.
- **PostgreSQL Integration**: Comprehensive tracking and monitoring of user activity through a *PostgreSQL* database layer, providing reliable persistence and analytical capabilities.
- **Scalable Cloud Deployment**: Containerized with *Docker* and deployed on *Google Cloud Platform* (GCP) for reliability and easy scaling.
- **WebAssembly Frontend**: Built with Dioxus and compiled to *WebAssembly* (WASM) for near-native performance in the browser with minimal resource usage.

### Technology Stack
* **Backend** - [*Axum*](https://docs.rs/axum/latest/axum/), [*Tokio*](https://tokio.rs/), [*Redis*](https://redis.io/),  [*PostgreSQL*](https://www.postgresql.org/)
* **Frontend** - [*Dioxus*](https://dioxuslabs.com/), [*WASM*](https://developer.mozilla.org/en-US/docs/WebAssembly)
* **Deployment** - [*Docker*](https://www.docker.com/), [*GCP*](https://cloud.google.com/)

# Project Structure
```
📦 rust-lob
├── backend/                  # Axum backend
│   ├── src/                  # Rust source files
│   │   ├── engine/           # Core orderbook logic
│   │   ├── file_upload/      # File parser and processor
│   │   ├── midwares/         # Middlewares and app states
│   │   ├── order_generator/  # Orderbook simulator
│   │   ├── route_handlers/   # Websocket and file-upload handlers
│   │   ├── main.rs           # Server entry point
│   ├── Cargo.toml            # Backend dependencies
│   ├── Dockerfile            # Backend Docker configuration
├── frontend/                 # Dioxus frontend
├── ├──assets/                # static assets
│   ├── src/                  # UI Components & Logic
│   │   ├── components/       # Page components
│   │   ├── pages/            # Route pages
│   │   ├── utils/            # Data and ui update handlers
│   │   ├── main.rs           # Dioxus entry point
│   ├── .env                  # Frontend environment variables
│   ├── build.rs              # Build script to inject env vars
│   ├── Cargo.toml            # Frontend dependencies
│   ├── Dioxus.toml           # Dioxus configs
├── README.md                 # Project documentation
├── LICENSE                   # MIT Open Source License
```

# Setup and deployment
1. **Prerequisites**  
Ensure you have the following installed
    - [Rust](https://www.rust-lang.org/tools/install)
    - [Docker](https://www.docker.com/)
    - [Google Cloud CLI](https://cloud.google.com/sdk/docs/install-sdk)
    - Postgres & [Redis](https://redis.io/docs/latest/develop/clients/) database instances with connection URLs
2. **Environment Variables**   
    **Backend**
    - `ORIGIN`- Origin URL for applying CORS policies
    - `REDIS` - Redis database connection url  
    - `POSTGRES` - Postgres database connection url  
    - `IP_LIMIT` - Maximum order limit per ip address  
    - `IP_WINDOW` - Time window for ip-level rate-limiting (**in seconds**) 
    - `GLOBAL_LIMIT` - Maximum order limit for entire application  
    - `GLOBAL_WINDOW` - Time window for global rate-limiting (**in seconds**) 
    - `HMAC_KEY` - the secret key for HMAC authentication  
   
   *NOTE*: You can alternatively inject these environment varaible using a `.env` file (like we do for frontend), but Cloud Run accepts environment variable during deployment for flexibility without rebuilding Docker images.  

    **Frontend**
    The frontend environment variables are defined in the `.env` file. These are read during build time using the `build.rs` script. For local development, API endpoint urls can use `http://127.0.0.1:7575/`.

3. **Running Locally**  
    **Backend**
    ```
    cd backend
    cargo run -r
    ```
    This spins up the server at `http://127.0.0.1:7575`.  
    
    **Frontend**   
    Install the `dioxus-cli` with
    ```
    cargo binstall dioxus-cli
    ```
    Run the development server:
    ```
    cd frontend
    dx serve
    ```
    Access the frontend at `127.0.0.1:8080`.

4. **Containerization and Cloud Deployment**  
    **Backend**  
    Build Docker image:
    ```
    docker build -t axum:v1 .
    ```
    Test containerized server:
    ```
    docker run -p 7575:7575 axum:v1
    ```
    Push to GCP Artifact Registry:
    ```
    docker tag axum:v1 <gcp-artifact-registry-repo-path>/axum:v1   
    docker push <gcp-artifact-registry-repo-path>/axum:v1
    ```
    where `<gcp-artifact-registry-repo-path>` is as discussed [here](https://cloud.google.com/artifact-registry/docs/docker/store-docker-container-images). Deploy to Cloud Run following the [Cloud Run deployment guide](https://cloud.google.com/run/docs/deploying).  

    **Frontend**  
    Bundle the app for production:
    ```
    dx bundle --platform web
    ```
    The WASM bundle will be saved in `frontend/target/dx/frontend/release/web/public`. This folder can be hosted using any cloud provider (GCP, AWS, or GitHub Pages).

# Contribution
Found a problem or have a suggestion? Feel free to open an issue or contribute by following these steps:
- Fork the repo and create a new branch: `git checkout -b feature-name`

- Make your changes and test thoroughly.

- Commit your changes: `git commit -m "feature-name"`

- Push to the branch: `git push origin feature-name`

- Submit a Pull Request for review.

# License
This project is licensed under the [MIT License](LICENSE).
