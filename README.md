## Abstract Substrate ETH RPC Adapter

Allows any Substrate chain to expose familiar ETH RPC without making any changes in the protocol level. The adapter supports the most essential ETH RPC calls and makes Substrate chains compatible with EVM development tools.

### Run

NOTE: light client support is experimental now so it's not guaranteed to be stable yet. This is due to it being [experimental](https://github.com/paritytech/subxt/blob/master/subxt/Cargo.toml#L71) in `subxt`.

This runs an adapter with `polkadot` by default:

```sh
cargo run
```

This runs an adapter with `polkadot` RPC node:

```sh
cargo run -- --url wss://polkadot.dotters.network
```

Alternatively, you can simply provide `--chain-spec` of the live chain (bootnodes included):

```sh
cargo run -- --chain-spec specs/kusama.json
```

For more options, run:

```sh
cargo run -- --help
```

### Docker

The adapter is available as a Docker image. You can run it with the following command:

```bash
docker build -t subeth-rpc-adapter .
docker run -p 8545:8545 subeth-rpc-adapter
```

### Run unit and integration tests

First we need to build the adapter:

```sh
cargo build
```

Then we can run the tests:

```sh
cargo test
```

This runs all tests, including unit and integration tests.

### Run integration tests with local node

Some integration tests require a running local Substrate node. These tests are marked as `#[ignore]` by default.

#### 1. Start the local Substrate node

First, build and run the test chain:

```sh
cd chain
cargo build --release
cargo run --release -- --dev
```

This starts a local Substrate node at `ws://127.0.0.1:9944`.

#### 2. Update local chain metadata (if needed)

The adapter uses compile-time metadata from `artifacts/local_metadata.scale`. If your local chain's runtime has changed, you need to regenerate this file:

```sh
# Option A: Using subxt CLI (recommended)
cargo install subxt-cli
subxt metadata --url ws://127.0.0.1:9944 -f bytes > artifacts/local_metadata.scale

# Option B: Using the test helper
cargo test test_fetch_local_metadata -- --ignored --nocapture

# Then rebuild the adapter with new metadata
cargo build
```

#### 3. Run local chain tests

With the local node running and metadata up-to-date:

```sh
# Run all ignored tests (local chain tests)
cargo test -- --ignored

# Run a specific ignored test
cargo test test_e2e_balance_transfer -- --ignored
```

**Note:** If you see `Metadata(IncompatibleCodegen)` errors, your `local_metadata.scale` doesn't match your running node. Follow step 2 to regenerate it.

### Run Polkadot integration tests

Tests that connect to live Polkadot network require the `polkadot` feature flag:

```sh
# Run Polkadot integration tests
cargo test --features polkadot

# Run specific Polkadot test
cargo test --features polkadot test_eth_rpc_url
cargo test --features polkadot test_eth_rpc_light_client
```

NOTE: due to light client support instability, `test_eth_rpc_light_client` can fail sometimes. This is due to the fact that light client is not always able to sync with the latest block. If you see this error, please try again.

### Using Docker Compose

Alternatively, you can use Docker Compose to run both the chain and adapter:

```sh
docker-compose up
```
