# Use the official Rust image as a base image
FROM rust:latest AS builder

# Set the working directory
WORKDIR /usr/src/subeth-rpc-adapter

# Copy the Cargo.toml and Cargo.lock files to cache dependencies
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs file to satisfy the build process
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# Build the dependencies (this step caches the dependencies)
RUN cargo build --release

# Remove the dummy main.rs file
RUN rm -rf src

# Copy the rest of the source code
COPY . .

# Build the project
RUN cargo build --release

# Use a minimal Debian-based image for the runtime
FROM debian:buster-slim

# Install necessary runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/subeth-rpc-adapter/target/release/subeth-rpc-adapter /usr/local/bin/subeth-rpc-adapter

# Set the entry point to run the binary
ENTRYPOINT ["subeth-rpc-adapter"]

# Expose the RPC port (default is 8545)
EXPOSE 8545
