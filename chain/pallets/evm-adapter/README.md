# EVM Adapter Pallet

This pallet provides EVM compatibility for Substrate chains by bridging Ethereum-style transactions with Substrate FRAME calls.

## Overview

The EVM Adapter pallet enables Substrate chains to process Ethereum transactions without requiring a full EVM execution environment. It works by:

1. **Receiving Ethereum Transactions**: Accepts transactions formatted as Ethereum EIP-1559 transactions
2. **Signature Verification**: Verifies ECDSA signatures and recovers the signer address
3. **Address Mapping**: Maps EVM addresses (AccountId20) to Substrate accounts (AccountId32) using blake2 hashing
4. **Call Decoding**: Interprets the transaction data and converts it to FRAME calls
5. **Dispatching**: Executes the decoded call on the Substrate chain

## Transaction Format

### Pallet Addressing

The `to` address in an Ethereum transaction represents a pallet name:
- First 8 characters of the pallet name are encoded as bytes
- Padded with zeros to create a valid 20-byte address
- Example: "Balances" becomes `0x42616c616e636573000000000000000000000000`

### Call Data Encoding

The `data` field contains:
- **First 4 bytes**: Function selector (keccak256 hash of function signature, first 4 bytes)
- **Remaining bytes**: ABI-encoded arguments

### Supported Calls

#### Balances Pallet

**Transfer** - `transfer(address,uint256)`
- Function selector: `0xa9059cbb`
- Arguments:
  - `address`: Destination address (32 bytes, last 20 bytes used)
  - `uint256`: Amount to transfer (32 bytes)
- Maps to: `pallet_balances::Call::transfer_allow_death`

## Example Usage

```javascript
// Using web3.js or ethers.js
const web3 = new Web3(rpcUrl);

// Balances pallet address
const balancesPalletAddress = "0x42616c616e636573000000000000000000000000";

// Encode transfer call
const transferData = web3.eth.abi.encodeFunctionCall({
  name: 'transfer',
  type: 'function',
  inputs: [{
    type: 'address',
    name: 'to'
  }, {
    type: 'uint256',
    name: 'value'
  }]
}, [recipientAddress, '1000000000000000000']); // 1 token

// Send transaction
const tx = {
  from: senderAddress,
  to: balancesPalletAddress,
  data: transferData,
  gas: 21000,
};

await web3.eth.sendTransaction(tx);
```

## Architecture

### Address Mapping

The pallet uses the same address mapping as the ETH RPC adapter:

```
EVM Address (20 bytes) -> Substrate Account (32 bytes):
  1. Pad address to 32 bytes (add 12 zero bytes at the end)
  2. Apply blake2_256 hash
  3. Use hash as AccountId32

Substrate Account (32 bytes) -> EVM Address (20 bytes):
  1. Take first 20 bytes of AccountId32
```

This ensures consistency between the adapter and the pallet.

### Signature Verification

The pallet verifies ECDSA signatures using secp256k1:
1. Reconstructs the message hash from transaction fields
2. Recovers the public key using `secp256k1_ecdsa_recover`
3. Derives the Ethereum address from the public key
4. Maps the address to a Substrate account for dispatch

## Extending the Pallet

To add support for additional pallets:

1. Add the pallet name to the `decode_call` function
2. Implement a decoder function (e.g., `decode_your_pallet_call`)
3. Map function selectors to FRAME calls
4. Return the constructed call

Example:

```rust
fn decode_call(transaction: &EthereumTransaction) -> Result<T::RuntimeCall, Error<T>> {
    let pallet_name = Self::pallet_name_from_address(transaction.to)
        .ok_or(Error::<T>::UnsupportedPallet)?;

    match pallet_name.as_slice() {
        b"Balances" => Self::decode_balances_call(transaction),
        b"YourPallet" => Self::decode_your_pallet_call(transaction),
        _ => Err(Error::<T>::UnsupportedPallet),
    }
}
```

## Integration with ETH RPC Adapter

This pallet is designed to work with the `subeth` ETH RPC adapter. The adapter forwards `eth_sendTransaction` and `eth_sendRawTransaction` requests to this pallet via the `transact` extrinsic.

## Limitations

- **Simplified RLP encoding**: Uses SCALE encoding for transaction hashing instead of proper RLP
- **Limited pallet support**: Currently only supports Balances pallet
- **No EVM bytecode execution**: This is a translation layer, not a full EVM
- **Signature format**: Simplified message hash construction

## Future Improvements

1. Add proper RLP encoding/decoding
2. Support more pallets (Staking, Democracy, etc.)
3. Add support for contract creation (integration with pallet-evm)
4. Implement transaction pool for unsigned transactions
5. Add gas metering and fee conversion
6. Support EIP-2930 and legacy transaction types

## License

MIT-0
