FROM rust:latest AS builder

WORKDIR /usr/src/subeth-rpc-adapter

RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    protobuf-compiler \
    clang \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY artifacts ./artifacts
COPY specs ./specs

RUN cargo build --release

FROM scratch

COPY --from=builder /usr/src/subeth-rpc-adapter/target/release/subeth-rpc-adapter /subeth-rpc-adapter

ENTRYPOINT ["/subeth-rpc-adapter"]

EXPOSE 8545
