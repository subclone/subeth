## SubEth Adapter

A proxy JSON RPC server that implements ETH RPC spec by forwarding requests to an inner Substrate light client or RPC server. It is designed to be generic over any Substrate-based chain, and light-client default.

### Features and quirks

One of the reasons for the existence of this adapter is to have an ETH RPC interface so that familiar ETH dev tools become instantly compatible with Substrate chains. For example, by running this sidecar adapter you can connect Metamask to a Substrate chain and read the state of the chain using Foundry tools, etc.

#### Pallet contract mapping

This adapter converts each pallet using this mapping logic: 

- `Balances` -> `0x{"Balances".to_bytes()} + 000000000000000000000000` => `0x62616c616e636573000000000000000000000000`

And this works vice versa, any RPC call (`eth_call`, i.e) that contains an `Address` parameter will be converted back using the same logic.

#### AccountId to Address conversion

For converting between most common `AccountId32` and ETH `Address` types, the adapter uses the following logic:

- `AccountId32` -> truncate last 12 bytes -> `Address`
- `Address` -> hash with `Blake2_256` -> `AccountId32`

This obviously means that there is currently no way to control mapped accounts from one another, but for the first iteration of the adapter, which is to make it READ operations compatible, this is enough. In the next iteration, there will be a trustless way to support write operations across different signature schemes.

#### Read Substrate chain's state

This adapter can read the state of the Substrate chain using the `eth_call` method. For example, reading the account state from `System` pallet, reading `Staking` pallet's storage items, etc.

The idea is to call the `eth_call` method with the following parameters:

- `to`: Converted pallet address, i.e using the above pallet contract mapping logic
- `input`: The encoded JSON string of the format:
  ```json
  {
    "name": "String",
    "keys": [
        "0x..",
        "...",
    ]
  }
  ```
    where `name` is the name of the storage item, and `keys` is an array of keys for the storage item. Adapter will handle all the rest of the logic of building and reading storage key, i.e gets hashers from the metadata, etc.

For example, reading the `System` pallet's `Account` storage item using `cast`:

```sh
cast rpc eth_call --to 0x73697374656d0000000000000000000000000000 --input '{"name":"Account","keys":["0x"+<AccountId32>]}' --rpc-url ws://localhost:8545
```

In the future, we can support calling Runtime API calls using the same logic.
