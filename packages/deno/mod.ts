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

  const args = [];
  if (chainSpec) args.push("--chain-spec", chainSpec);
  if (url) args.push("--url", url);
  args.push("--chain-id", chainId.toString());
  args.push("--port", port.toString());

  const process = Deno.run({
    cmd: [binaryPath, ...args],
    stdout: "piped",
    stderr: "piped",
  });

  return {
    url: `http://localhost:${port}`,
    process,
    stop: async () => {
      process.close();
      await Deno.remove(binaryPath);
    },
  };
}

if (import.meta.main) {
  const adapter = await startSubeth({ chainSpec: "spec.json" });
  console.log(`Running at ${adapter.url}`);
  // await adapter.stop(); // Uncomment to stop after testing
}
