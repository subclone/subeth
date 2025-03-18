## Abstract Substrate ETH RPC Adapter

Allows any Substrate chain to expose familiar ETH RPC without making any changes in the protocol level. The adapter supports the most essential ETH RPC calls and makes Substrate chains compatible with EVM development tools.

### Docker

The adapter is available as a Docker image. You can run it with the following command:

```bash
docker build -t subeth-rpc-adapter .
docker run -p 8545:8545 subeth-rpc-adapter
```

