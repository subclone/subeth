# Subeth Simple Transfer Demo

A minimal demo application showcasing token transfers on Substrate chains using MetaMask and the Subeth ETH RPC adapter.

## Features

- Connect MetaMask wallet
- View account address, balance, and chain ID
- Send token transfers using Ethereum-style transactions
- Real-time balance updates
- Transaction status feedback

## Prerequisites

1. **MetaMask** browser extension installed
2. **Subeth Adapter** running on `http://localhost:8545`
3. **Substrate Chain** with funded accounts

## Quick Start

1. **Install dependencies**:
   ```bash
   npm install
   ```

2. **Start development server**:
   ```bash
   npm run dev
   ```

3. **Open browser**: Navigate to `http://localhost:5173` (or the port shown in terminal)

4. **Connect MetaMask**:
   - Click "Connect MetaMask"
   - Approve the connection
   - Your address and balance will be displayed

5. **Send a transfer**:
   - Enter recipient address (0x...)
   - Enter amount in smallest unit (e.g., 1000000000000)
   - Click "Send Transaction"
   - Approve in MetaMask

## Network Configuration

The app connects to a local Subeth adapter by default:

- **RPC URL**: `http://localhost:8545`
- **Chain ID**: 42 (0x2A)
- **Currency**: UNIT (18 decimals)

To use a different network, modify the `SUBETH_NETWORK` constant in `src/main.js`.

## Running the Subeth Stack

### Start the Substrate chain:
```bash
cd chain
cargo run --release
```

### Start the ETH RPC adapter:
```bash
cd adapter
cargo run --release
```

### Serve the dapp:
```bash
cd simple-dapp
npm run dev
```

## How It Works

1. **Connection**: MetaMask connects to the Subeth adapter at `http://localhost:8545`
2. **Balance Query**: App uses `eth_getBalance` to fetch account balance from Substrate chain
3. **Transfer**: User signs an Ethereum transaction with MetaMask
4. **Processing**: Subeth adapter forwards the transaction to the Substrate chain (requires `evm-adapter` pallet)
5. **Confirmation**: Transaction hash is returned and balance updates

## Key Functions

- `connectWallet()` - Request MetaMask connection
- `updateAccountInfo()` - Fetch balance and chain ID
- `sendTransfer()` - Submit transfer transaction
- `formatBalance()` - Convert wei to readable format

## Transaction Format

Transfers use standard Ethereum transaction structure:
```javascript
{
  from: currentAccount,
  to: recipient,
  value: valueHex,  // Amount in hex
  gas: '0x5208'     // 21000 gas
}
```

## Troubleshooting

**MetaMask not connecting**:
- Ensure adapter is running on port 8545
- Check browser console for errors

**Balance shows 0**:
- Verify account is funded on the Substrate chain
- Check adapter is synced

**Transaction fails**:
- Ensure `evm-adapter` pallet is in runtime
- Check sufficient balance for transfer + gas
- Verify recipient address is valid

**Wrong chain**:
- Check MetaMask is connected to correct network
- Use "Switch Network" in MetaMask if needed

## Build for Production

```bash
npm run build
```

Outputs to `dist/` directory. Serve with any static file server.

## Tech Stack

- Vite - Build tool
- Vanilla JavaScript - No frameworks
- MetaMask - Wallet integration
- Subeth - ETH RPC adapter for Substrate

## Learn More

- [Subeth Documentation](https://github.com/dastansam/subeth)
- [MetaMask Documentation](https://docs.metamask.io/)
- [Ethereum JSON-RPC](https://ethereum.org/en/developers/docs/apis/json-rpc/)
