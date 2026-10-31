import { assertEquals } from "@std/assert";
import { buildArgs } from "./main.ts";

Deno.test(function passesPortAsRpcPort() {
  // The adapter only understands --rpc-port; a wrong flag makes the binary exit immediately
  assertEquals(buildArgs({ chainId: 42, port: 9000 }), [
    "--chain-id",
    "42",
    "--rpc-port",
    "9000",
  ]);
});

Deno.test(function prefersExplicitSources() {
  assertEquals(
    buildArgs({ url: "ws://127.0.0.1:9944", chainId: 7, port: 8545 }),
    ["--url", "ws://127.0.0.1:9944", "--chain-id", "7", "--rpc-port", "8545"],
  );
});
