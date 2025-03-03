FROM rust:1.84.0 AS builder

WORKDIR /usr/src/app/backend

RUN apt-get update && apt-get install -y \
    musl-tools \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./src ./src

RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --target x86_64-unknown-linux-musl --release

FROM debian:bullseye-slim
WORKDIR /usr/src/app

RUN apt-get update && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/backend/target/x86_64-unknown-linux-musl/release/backend /usr/src/app/

EXPOSE 7575

CMD ["./backend"]