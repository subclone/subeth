# --- Builder Stage ---
# Use a specific Debian version (Bookworm) for consistency
FROM rust:1.86-bookworm AS builder

WORKDIR /usr/src/subeth-rpc-adapter

# Install build dependencies - NO musl-tools needed
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    protobuf-compiler \
    # clang or gcc is fine, cc-rs usually detects correctly
    clang \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY artifacts ./artifacts
COPY specs ./specs

# Standard release build, links against glibc in the builder
# Add --verbose if you need more build output details
RUN cargo build --release --verbose

# --- Final Stage ---
# Use the corresponding minimal Debian version
FROM debian:bookworm-slim

# Install runtime dependencies if any (e.g., libssl if not statically linked by openssl-sys feature)
# Check if your app needs libssl at runtime: ldd target/release/subeth-rpc-adapter in builder
# If it lists libssl.so, uncomment the next line:
# RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*

# Copy necessary files from the builder
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /usr/src/subeth-rpc-adapter/target/release/subeth /usr/local/bin/subeth-rpc-adapter

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/subeth-rpc-adapter"]

# Expose the port
EXPOSE 8545
