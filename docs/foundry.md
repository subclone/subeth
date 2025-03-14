## Foundry queries

Commands to test ETH RPC exposed from `subeth` adapter.

### Run subeth adapter

This runs an adapter with `polkadot` light client.

```sh 
cargo run
```

This runs an adapter with `polkadot` RPC node.

```sh
cargo run -- --url wss://polkadot.dotters.network
```

Alternatively, you can simply provide `--chain-spec` of the live chain (bootnodes included):

```sh
cargo run -- --chain-spec specs/kusama.json
```

### Get block by number

Latest:

```sh
cast rpc eth_blockNumber --rpc-url ws://localhost:8545
```

By number:

```sh
cast rpc eth_getBlockByNumber --rpc-url ws://localhost:8545 latest false
```

### Get block by hash

First, fetch the latest block hash:

```sh
cast rpc eth_getBlockByNumber --rpc-url ws://localhost:8545 latest false
```

Then, use the hash to fetch the block:

```sh
cast rpc eth_getBlockByHash --rpc-url ws://localhost:8545 <block-hash> false
```

### Get transaction by block and index

First, fetch the latest block hash:

```sh
cast rpc eth_getBlockByNumber --rpc-url ws://localhost:8545 latest false
```

Then, use the hash to fetch the transaction:

```sh
cast rpc eth_getTransactionByBlockNumberAndIndex --rpc-url ws://localhost:8545 <block-hash> 0
```
