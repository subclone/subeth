import { existsSync } from "https://deno.land/std@0.208.0/fs/mod.ts";

export async function startSubeth({
  chainSpec,
  url,
  chainId = 42,
  port = 8545,
}: {
  chainSpec?: string;
  url?: string;
  chainId?: number;
  port?: number;
} = {}) {
  const os = Deno.build.os;
  let binaryName: string;

  if (os === "darwin") {
    binaryName = "subeth-macos";
  } else if (os === "linux") {
    binaryName = "subeth-ubuntu";
  } else {
    throw new Error("Unsupported OS. Only macOS and Ubuntu are supported.");
  }

  const binaryPath = `./${binaryName}`;
  if (!existsSync(binaryPath)) {
    const url = `https://github.com/subclone/subeth/releases/latest/download/${binaryName}`;
    const response = await fetch(url);
    if (!response.ok) throw new Error("Failed to fetch binary");
    const binary = await response.arrayBuffer();
    await Deno.writeFile(binaryPath, new Uint8Array(binary));
    await Deno.chmod(binaryPath, 0o755); // Make executable
  }

  // `Deno.run` was removed in Deno 2; `Deno.Command` is the supported API
  const process = new Deno.Command(binaryPath, {
    args: buildArgs({ chainSpec, url, chainId, port }),
    stdout: "piped",
    stderr: "piped",
  }).spawn();

  return {
    url: `http://localhost:${port}`,
    process,
    stop: async () => {
      process.kill();
      await process.status;
      await Deno.remove(binaryPath);
    },
  };
}

/** Command line arguments for the subeth binary */
export function buildArgs({
  chainSpec,
  url,
  chainId,
  port,
}: {
  chainSpec?: string;
  url?: string;
  chainId: number;
  port: number;
}): string[] {
  const args: string[] = [];
  if (chainSpec) args.push("--chain-spec", chainSpec);
  if (url) args.push("--url", url);
  args.push("--chain-id", chainId.toString());
  // The adapter takes `--rpc-port` (from sc-cli); `--port` is rejected by the CLI parser
  args.push("--rpc-port", port.toString());
  return args;
}

if (import.meta.main) {
  const adapter = await startSubeth({ chainSpec: "spec.json" });
  console.log(`Running at ${adapter.url}`);
  // await adapter.stop(); // Uncomment to stop after testing
}
