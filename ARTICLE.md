# Subeth: A Different Approach to EVM Compatibility for Substrate Chains

If you've worked in the Polkadot ecosystem, you've probably encountered the challenge of making Substrate chains accessible to Ethereum developers and tools. The most common solution, Frontier, requires embedding an entire EVM runtime into your chain. But what if there was a lighter way?

## The Problem

Ethereum has the largest developer ecosystem in Web3. Millions of developers know Solidity, use MetaMask, and rely on tools like Foundry and web3.js. Meanwhile, Substrate offers superior architecture and flexibility but requires learning new paradigms and tools.

Existing solutions typically require:
- Major runtime modifications
- Embedding EVM execution engines
- Custom client implementations
- Modified developer tooling

This complexity creates a high barrier for chains wanting Ethereum compatibility and for developers wanting to interact with Substrate chains.

## A Simpler Approach

Subeth takes a different path: instead of bringing the EVM to Substrate, it translates Ethereum's language to Substrate's.

The architecture is straightforward:
1. A standalone adapter service that speaks Ethereum JSON-RPC
2. An optional runtime pallet for write operations
3. No changes to the Substrate client
4. No embedded EVM execution engine

When you connect MetaMask to a Subeth-enabled chain, it just works. The adapter translates `eth_getBalance` to the equivalent Substrate RPC call. From MetaMask's perspective, it's talking to an Ethereum node. From the Substrate chain's perspective, nothing unusual is happening.

## How It Works

**Reading State**: The adapter maps each pallet to a unique Ethereum address. Want to query the Balances pallet? It gets a deterministic address like `0x42616c616e636573...`. Use standard `eth_call` to read any storage item from any pallet. Your familiar Ethereum tools now read Substrate state.

**Writing Transactions**: This requires a small runtime pallet. It verifies Ethereum signatures, maps addresses between formats, and dispatches calls. Sign a transaction with MetaMask, and the pallet executes it on-chain. The pallet is lightweight—no EVM execution, just signature verification and call dispatch.

**Trustless by Default**: The adapter runs an embedded light client (smoldot) for trustless access. No need to trust external RPC nodes. You can optionally connect to a remote node for speed, but the default is secure.

## What Makes This Different

**Frontier** is comprehensive but heavy. It embeds a full EVM runtime and modifies both node and client. Great if you need 100% EVM compatibility, but overkill if you just want Ethereum tools to work.

**Acala EVM+** takes a similar approach but requires custom tooling (`bodhi.js` instead of `web3.js`) and is tailored to Acala Network.

**Polkamask** is a browser extension that helps MetaMask work with Substrate, but it's MetaMask-specific and requires trusting a plugin.

**Subeth** is deliberately minimal. Standard tools. No custom plugins. Works with any Substrate chain. Read access needs zero runtime changes. Write access needs one small pallet.

## Real-World Use Cases

**Block Explorers**: Build Ethereum-style explorers for Substrate chains using existing indexer infrastructure. No need to learn Substrate-specific APIs.

**Data Analytics**: Query Substrate chain state using your existing Ethereum analytics tools and pipelines.

**Developer Onboarding**: Let Ethereum developers interact with your Substrate chain using tools they already know before investing time to learn Substrate-native development.

**Multi-Chain Apps**: Build applications that work across Ethereum and Substrate chains using a single codebase and toolset.

**Testing and Development**: Use Foundry, Hardhat, or other Ethereum development tools to test interactions with Substrate chains.

## Trade-offs

This isn't "full" EVM compatibility. You can't deploy Solidity contracts (unless you also add pallet-evm). Transaction hashes work differently. Gas is estimated rather than precisely calculated. Account models require mapping.

But that's the point. Not every chain needs full EVM compatibility. Sometimes you just want Ethereum tools to work with your existing Substrate runtime. Subeth optimizes for developer experience and minimal integration cost, not for running Uniswap clones.

## Getting Started

The stack runs in three commands:
```bash
# Start the chain
cargo run --release

# Start the adapter
cargo run --release -- --rpc-url ws://localhost:9944

# Or use Docker
docker-compose up
```

Point MetaMask at `http://localhost:8545` and you're reading Substrate state through Ethereum RPC. Add the adapter pallet to your runtime and you can send transactions too.

The demo dapp shows it working: connect MetaMask, check your balance, send a transfer. Standard Ethereum UX, Substrate chain underneath.

## What's Next

The current implementation proves the concept works. Future improvements could include:
- Removing the pallet requirement by handling more logic in the adapter
- Expanding RPC method coverage (logs, filters, more subscription types)
- Better gas estimation using runtime APIs
- Multi-chain proxy service for accessing multiple Substrate chains through one adapter

The goal isn't to replace Frontier or become the definitive EVM solution. It's to provide an alternative for chains that want Ethereum tool compatibility without the complexity of full EVM integration.

## Try It

Subeth is open source and ready to test:
- **Code**: [github.com/dastansam/subeth](https://github.com/dastansam/subeth)
- **Quick Start**: Clone, run `docker-compose up`, open the demo at `localhost:3000`
- **Documentation**: See INSTRUCTIONS.md for architecture details

If you're building a Substrate chain and want Ethereum developers to easily interact with it, or if you're an Ethereum developer curious about Substrate, give it a try.

The Polkadot ecosystem benefits from multiple approaches to EVM compatibility. Subeth adds a lightweight option to the toolkit.

---

*This project was built with support from the Web3 Foundation Grants Program.*
