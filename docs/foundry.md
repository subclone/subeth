## Foundry queries

Commands to test ETH RPC exposed from `subeth` adapter.

### Run subeth adapter

This runs an adapter on `polkadot`:

```sh 
cargo run
```

This runs an adapter with custom `polkadot` RPC node.

```sh
cargo run -- --url wss://polkadot.dotters.network
```

Alternatively, you can simply provide `--chain-spec` of the live chain (bootnodes included):

NOTE: light client support is experimental now so it's not guaranteed to be stable yet. This is due to it being experimental in `subxt`.

```sh
cargo run -- --chain-spec specs/kusama.json
```

### Chain info

```sh
cast chain-id --rpc-url ws://localhost:8545
```

### Get balance

```sh
cast balance --rpc-url ws://localhost:8545 0x3cF363cEBF82552bBc16b846eaD5fe0B416519F5
```


### Get transaction count

```sh
cast rpc eth_getTransactionCount --rpc-url ws://localhost:8545 0x3cF363cEBF82552bBc16b846eaD5fe0B416519F5 latest
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
cast rpc eth_getTransactionByBlockNumberAndIndex --rpc-url ws://localhost:8545 <block-number> 0
```


### Read pallet's state

```sh
cast rpc eth_call '{
    "to": "0x53797374656d0000000000000000000000000000",
    "data": "7b226e616d65223a224163636f756e74222c226b657973223a5b5b39342c38312c39372c31302c3134332c39312c39342c342c3139312c34372c34302c3135302c31302c3136312c35362c3232332c34332c3131372c34392c3230382c3231392c3233342c3134372c39392c35342c3233372c36342c3136322c342c3230312c34362c31385d5d7d",
  }' --rpc-url ws://localhost:8545
```

where:
- `to` is the converted pallet address for `System`
- `data` is the encoded JSON string of the format:
  - {
    "name": String,
    "keys": Vec<Bytes>
    }
  e.g, in the case above
  {
    "name":"Account",
    "keys":[[94,81,97,10,143,91,94,4,191,47,40,150,10,161,56,223,43,117,49,208,219,234,147,99,54,237,64,162,4,201,46,18]]
  }

For storage value `Staking` and `ActiveEra`:


```sh
cast rpc eth_call '{
    "to": "0x5374616b696e6700000000000000000000000000",
    "data": "7b226e616d65223a22416374697665457261222c226b657973223a5b5d7d",
  }' --rpc-url ws://localhost:8545
```

to get the `data`, simply conert the string to bytes

```sh
echo -n '{"name":"ActiveEra","keys":[]}' | xxd -p -c 1000
```
