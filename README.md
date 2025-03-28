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

NOTE: due to light client support instability, integration test `test_light_client` can fail sometimes. This is due to the fact that light client is not always able to sync with the latest block. If you see this error, please try again.
