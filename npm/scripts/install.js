const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const https = require("node:https");

const packageRoot = path.join(__dirname, "..");
const packageVersion = require(path.join(packageRoot, "package.json")).version;
const vendorDirectory = path.join(packageRoot, "vendor");
const platforms = {
  "linux:x64": ["probelm-linux-x86_64.tar.gz", "tar"],
  "linux:arm64": ["probelm-linux-aarch64.tar.gz", "tar"],
  "darwin:x64": ["probelm-macos-x86_64.tar.gz", "tar"],
  "darwin:arm64": ["probelm-macos-aarch64.tar.gz", "tar"],
  "win32:x64": ["probelm-windows-x86_64.zip", "zip"],
  "win32:arm64": ["probelm-windows-aarch64.zip", "zip"],
};

const target = platforms[`${process.platform}:${process.arch}`];
if (!target) throw new Error(`Unsupported probelm platform: ${process.platform}/${process.arch}`);

function download(url, destination) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, { headers: { "User-Agent": "probelm-npm-installer" } }, (response) => {
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        response.resume();
        download(new URL(response.headers.location, url), destination).then(resolve, reject);
        return;
      }
      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`Download failed (${response.statusCode}): ${url}`));
        return;
      }
      const output = fs.createWriteStream(destination);
      response.pipe(output);
      output.on("finish", () => output.close(resolve));
      output.on("error", reject);
    });
    request.on("error", reject);
  });
}

function checksum(file) {
  return new Promise((resolve, reject) => {
    const hash = crypto.createHash("sha256");
    const input = fs.createReadStream(file);
    input.on("error", reject);
    input.on("data", (chunk) => hash.update(chunk));
    input.on("end", () => resolve(hash.digest("hex")));
  });
}

async function main() {
  if (process.argv.includes("--selftest")) {
    console.log(`probelm npm installer supports ${Object.keys(platforms).length} platform/architecture combinations`);
    return;
  }

  const [archiveName, archiveType] = target;
  const baseUrl = `https://github.com/keton-id/probelm/releases/download/v${packageVersion}`;
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "probelm-install-"));
  const archivePath = path.join(tempRoot, archiveName);
  const checksumPath = `${archivePath}.sha256`;
  try {
    fs.mkdirSync(vendorDirectory, { recursive: true });
    await download(`${baseUrl}/${archiveName}`, archivePath);
    await download(`${baseUrl}/${archiveName}.sha256`, checksumPath);
    const expected = fs.readFileSync(checksumPath, "utf8").trim().split(/\s+/)[0].toLowerCase();
    const actual = await checksum(archivePath);
    if (!/^[a-f0-9]{64}$/.test(expected) || expected !== actual) {
      throw new Error(`Checksum verification failed for ${archiveName}`);
    }

    if (archiveType === "tar") {
      execFileSync("tar", ["-xzf", archivePath, "-C", vendorDirectory]);
    } else {
      execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command",
        `Expand-Archive -LiteralPath '${archivePath.replaceAll("'", "''")}' -DestinationPath '${vendorDirectory.replaceAll("'", "''")}' -Force`]);
    }

    const binary = path.join(vendorDirectory, process.platform === "win32" ? "probelm.exe" : "probelm");
    if (!fs.existsSync(binary)) throw new Error(`Release archive did not contain ${path.basename(binary)}`);
    if (process.platform !== "win32") fs.chmodSync(binary, 0o755);
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(`probelm npm install failed: ${error.message}`);
  process.exit(1);
});
