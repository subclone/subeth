# Subeth

## Project Overview

**Subeth** is an alternative EVM compatibility solution for Substrate chains that requires minimal runtime and client changes. It consists of two main components:

1. **ETH RPC Adapter** - Standalone service translating Ethereum JSON-RPC calls to Substrate equivalents
2. **EVM Adapter Pallet** - Optional runtime pallet enabling write operations from Ethereum transactions

## Core Architecture

### 1. ETH RPC Adapter

**Purpose**: Enable read access to Substrate chains using Ethereum tools (MetaMask, web3.js, ethers.js, Foundry)

**Key Design Decisions**:

- **Embedded Light Client**: Uses `smoldot` by default for trustless access, avoiding dependency on external RPC nodes
- **Alternative RPC Mode**: Can connect to remote Substrate node (faster but less secure)
- **No Client Modifications**: Runs as separate service, no changes to Substrate node required

**Pallet Addressing Scheme**:
```rust
// Each pallet gets deterministic address
// "Balances" → 0x42616c616e636573000000000000000000000000
// First 8 bytes of pallet name, padded to 20 bytes
```

**Supported RPC Methods**:
- **Client Info**: `eth_chainId`, `eth_protocolVersion`, `eth_syncing`, `eth_accounts`
- **Blocks**: `eth_blockNumber`, `eth_getBlockByNumber`, `eth_getBlockByHash`
- **State**: `eth_getBalance`, `eth_getStorageAt`, `eth_getCode`, `eth_getTransactionCount`
- **Calls**: `eth_call` (reads pallet storage via JSON payload)
- **Gas**: `eth_gasPrice`
- **Subscriptions**: `eth_subscribe` (newHeads)
- **Transactions**: `eth_sendTransaction`, `eth_sendRawTransaction` (requires pallet)

**Storage Query Mechanism**:

Uses `eth_call` with JSON payload to read any pallet storage:
```json
{
  "to": "0x42616c616e636573...",  // Pallet address
  "data": {
    "name": "TotalIssuance",       // Storage item
    "keys": [[account_bytes]]      // Optional keys for maps
  }
}
```

### 2. EVM Adapter Pallet

**Purpose**: Enable write operations from Ethereum-signed transactions

**Core Functionality**:
1. Accept EIP-1559 Ethereum transactions
2. Verify ECDSA signatures and recover signer (H160)
3. Map H160 address to AccountId32
4. Decode transaction data as SCALE-encoded RuntimeCall
5. Dispatch call with mapped account as origin

**Account Mapping**:
- **H160 → AccountId32**: Blake2-256 hash of H160
- **AccountId32 → H160**: Truncate to first 20 bytes

**Why Not Pure Adapter?**
- Substrate and Ethereum have fundamental differences (signature schemes, account models)
- Pallet handles signature verification, account mapping, and call dispatch
- Opt-in: chains can use adapter-only for read access

**Transaction Flow**:
```
MetaMask → Sign EIP-1559 TX → Adapter → Pallet → Verify Signature →
Map Address → Decode SCALE Call → Dispatch with Origin
```

## Implementation Details

### Adapter [`/adapter`](./adapter)

**Key Files**:
- [`src/server.rs`](./adapter/src/server.rs) - ETH RPC server implementation
- [`src/sub_client.rs`](./adapter/src/sub_client.rs) - Substrate light client wrapper (smoldot)
- [`src/adapter.rs`](./adapter/src/adapter.rs) - Type conversions (Ethereum ↔ Substrate)
- [`src/traits.rs`](./adapter/src/traits.rs) - ETH RPC trait definitions

**Dependencies**:
- `jsonrpsee` - JSON-RPC server
- `smoldot-light` - Embedded light client
- `alloy` - Ethereum types
- `sp_core`, `sp_runtime` - Substrate primitives

### Pallet [`/chain/pallets/evm-adapter`](./chain/pallets/evm-adapter)

**Key Extrinsic**:
```rust
#[pallet::call]
pub fn transact(
    origin: OriginFor<T>,
    transaction: EthereumTransaction,
) -> DispatchResult {
    // 1. Verify ECDSA signature
    let signer_h160 = recover_signer(&transaction)?;

    // 2. Map to AccountId32
    let account_id = Self::h160_to_account_id32(signer_h160);

    // 3. Decode SCALE-encoded RuntimeCall
    let call = decode_call(transaction.data)?;

    // 4. Dispatch with signed origin
    call.dispatch(RawOrigin::Signed(account_id).into())
}
```

**Storage**: Minimal - primarily stateless transaction processing

### Primitives [`/chain/primitives`](./chain/primitives)

**Shared Types**:
- `EthereumTransaction` - EIP-1559 transaction structure
- Type conversions between `alloy` and `sp_core`

### Test Chain [`/chain`](./chain)

**Purpose**: Demonstrate integration

**Pallets**:
- Standard: System, Balances, Timestamp, Sudo
- Custom: `pallet-evm-adapter`

## Limitations

**Known Constraints**:
1. **Transaction Hashes**: Substrate tx hashes not unique - adapter generates deterministic alternatives
2. **Gas Model**: Substrate weight ≠ Ethereum gas - adapter provides estimates
3. **Account Model**: Requires pre-funding Substrate account before EVM-style interaction
4. **Partial Compatibility**: Not "full" EVM - prioritizes practical compatibility over 100% spec compliance

**Design Trade-offs**:
- Chose substrate-like behavior where conflicts exist (better primitives)
- Focus on developer experience over perfect EVM emulation
- Lightweight over feature-complete

## DApp Demo

**Location**: [`/simple-dapp`](./simple-dapp)

**Features Demonstrated**:
1. MetaMask connection to Substrate chain
2. Reading pallet storage (Balances, System, Staking)
3. Block explorer with Ethereum-style formatting
4. Account address conversion (H160 ↔ AccountId32)
5. Balance queries and transaction sending
6. Raw RPC playground

**Technology**: Vite

## Testing

### Unit Tests
- Adapter: [`/adapter/src/tests.rs`](./adapter/src/tests.rs)
- Pallet: [`/chain/pallets/evm-adapter/src/tests.rs`](./chain/pallets/evm-adapter/src/tests.rs)

### Integration Tests
- Full RPC call coverage
- Block queries, balance checks, storage reads
- Transaction submission with pallet

### End-to-End
- DApp demonstrates real-world usage
- MetaMask integration
- Multiple tool compatibility (web3.js, ethers.js, Foundry)

## Running the Project

### Quick Start
```bash
# 1. Start test chain
cd chain
cargo run --release

# 2. Start adapter
cd adapter
cargo run --release -- --chain-spec ../chain/chainspec.json
```

### With Docker
```bash
docker-compose up
```

## Conclusion

Subeth achieves its core goal: **minimal-change EVM compatibility for Substrate chains**.

**Key Achievements**:
- Read access works with zero runtime changes
- Write access requires single lightweight pallet
- No client modifications needed
- Compatible with standard Ethereum tooling
- Trustless operation via embedded light client
- Chain-agnostic design

**Innovation**: Proves Substrate chains can be accessed via Ethereum tools without embedding full EVM execution engine, lowering barrier to entry for developers and tools.
