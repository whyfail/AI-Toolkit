import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const version = process.argv[2];
const semverPattern = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;

if (!version || !semverPattern.test(version)) {
  console.error("Usage: pnpm version:sync <x.y.z>");
  process.exit(1);
}

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

async function updateJsonVersion(relativePath) {
  const filePath = path.join(repoRoot, relativePath);
  const content = await readFile(filePath, "utf8");
  const versionPattern = /^(\s*"version"\s*:\s*)".*"(,?)$/m;

  if (!versionPattern.test(content)) {
    throw new Error(`Could not find version in ${relativePath}`);
  }

  await writeFile(
    filePath,
    content.replace(versionPattern, `$1"${version}"$2`)
  );
}

async function updateCargoToml() {
  const filePath = path.join(repoRoot, "src-tauri/Cargo.toml");
  const content = await readFile(filePath, "utf8");
  const versionPattern = /^(\s*)version = ".*"$/m;

  if (!versionPattern.test(content)) {
    throw new Error("Could not find package version in src-tauri/Cargo.toml");
  }

  const updated = content.replace(
    versionPattern,
    `$1version = "${version}"`
  );

  await writeFile(filePath, updated);
}

await updateJsonVersion("package.json");
await updateJsonVersion("src-tauri/tauri.conf.json");
await updateCargoToml();

console.log(`Synced project version to ${version}`);
