import { rm, readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const appRoot = path.resolve(__dirname, "..");
const tauriConfigPath = path.join(appRoot, "src-tauri", "tauri.conf.json");
const buildRoot = path.join(appRoot, "src-tauri", "gen", "apple", "build");

async function readProductName() {
  const raw = await readFile(tauriConfigPath, "utf8");
  const config = JSON.parse(raw);
  return config.productName ?? "App";
}

async function cleanExistingIosApps(productName) {
  try {
    const entries = await readdir(buildRoot, { withFileTypes: true });
    await Promise.all(
      entries
        .filter((entry) => entry.isDirectory() && !entry.name.endsWith(".xcarchive"))
        .map((entry) =>
          rm(path.join(buildRoot, entry.name, `${productName}.app`), {
            recursive: true,
            force: true,
          }),
        ),
    );
  } catch (error) {
    if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
      return;
    }
    throw error;
  }
}

async function main() {
  const productName = await readProductName();
  await cleanExistingIosApps(productName);
  const forwardedArgs = process.argv.slice(2);
  const tauriArgs =
    forwardedArgs[0] === "--" ? forwardedArgs.slice(1) : forwardedArgs;

  const child = spawn(
    "pnpm",
    ["exec", "tauri", "ios", "build", ...tauriArgs],
    {
      cwd: appRoot,
      stdio: "inherit",
      env: process.env,
    },
  );

  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
