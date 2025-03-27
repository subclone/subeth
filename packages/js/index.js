const { existsSync } = require("fs");
const { promisify } = require("util");
const { execFile } = require("child_process");
const { unlink } = require("fs/promises");
const { spawn } = require("node:child_process");

async function startSubeth({ chainSpec, url, chainId = 42, port = 8545 } = {}) {
  const os = require("process").platform;
  let binaryName;

  if (os === "darwin") {
    binaryName = "subeth-macos";
  } else if (os === "linux") {
    binaryName = "subeth-ubuntu";
  } else {
    throw new Error("Unsupported OS. Only macOS and Ubuntu are supported.");
  }

  const binaryPath = `./${binaryName}`;
  if (!existsSync(binaryPath)) {
    const response = await fetch(
      `https://github.com/subclone/subeth/releases/latest/download/${binaryName}`
    );
    if (!response.ok) throw new Error("Failed to fetch binary");
    const binary = await response.arrayBuffer();
    require("fs").writeFileSync(binaryPath, Buffer.from(binary));
    require("fs").chmodSync(binaryPath, "755");
  }

  const args = [];
  if (chainSpec) args.push("--chain-spec", chainSpec);
  if (url) args.push("--url", url);
  args.push("--chain-id", chainId.toString());
  // args.push("--port", port.toString());

  const process = spawn(binaryPath, args, { stdio: "pipe" });

  return {
    url: `http://localhost:${port}`,
    process,
    stop: async () => {
      process.kill();
      await unlink(binaryPath);
    },
  };
}

module.exports = { startSubeth };

if (require.main === module) {
  startSubeth()
    .then((adapter) => {
      adapter.process.on("error", (error) => {
        console.error("Process error:", error);
      });
      adapter.process.on("exit", (code) => {
        console.log(`Process exited with code ${code}`);
        process.exit(code);
      });
      console.log(`Running at ${adapter.url}`);
    })
    .catch((err) => console.error(err));
}
