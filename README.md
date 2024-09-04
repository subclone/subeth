## Substrate ETH-RPC Adapter

This project aims to create an adapter that will allow Substrate chains to be used with existing Ethereum tools. The adapter will support the most essential ETH RPC calls to make it work with the existing tools. Main goals here will be ability to connect to Metamask, `web3.js`, read Substrate chain's pallets' state and support subscriptions. Adapter will have option of running local `smoldot` instance or connecting to remote RPC node.

### Run

```rs
cargo run --release
```

| **0b.** | Documentation | We will provide both **inline documentation** of the code and a basic **tutorial** that explains how a user can (for example) spin up the adapter and connect it to the familiar developer tools. |
| **0c.** | Testing and Testing Guide | Core functions will be fully covered by comprehensive unit tests to ensure functionality and robustness. In the guide, we will describe how to run these tests. |
| **0d.** | Docker | We will provide a Dockerfile(s) that can be used to test all the functionality delivered with this milestone. |
| 1. | ETH-RPC Adapter | We will create a generic ETH-RPC Adapter service for Substrate chains. It will support the most essential ETH RPC calls to make it work with the existing tools. Main goals here will be ability to connect to Metamask, `web3.js`, read Substrate chain's pallets' state and support subscriptions. Adapter will have option of running local `smoldot` instance or connecting to remote RPC node. |
| 2. | Deno module | We will create a Deno module that can connect to Substrate chain as an ETH-RPC adapter + light client. |
| 3. | Javascript package | We will provide a javascript package that can connect to Substrate chain as an ETH-RPC adapter + light client. |
| 4. | End-to-end tests | We will provide comprehensive end-to-end tests for the adapter. |
