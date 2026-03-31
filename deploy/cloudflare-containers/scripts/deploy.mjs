#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const deployDir = resolve(__dirname, "..");
const repoRoot = resolve(deployDir, "..", "..");
const baseConfigPath = join(deployDir, "wrangler.jsonc");
const repoDockerfilePath = join(repoRoot, "Dockerfile");
const cloudflareDockerfilePath = join(repoRoot, "Dockerfile.cloudflare");

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: deployDir,
    encoding: "utf8",
    stdio: ["inherit", "pipe", "pipe"],
    ...options,
  });

  if (result.error) {
    throw result.error;
  }

  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const combined = [stdout, stderr].filter(Boolean).join("\n").trim();

  if (result.status !== 0) {
    const error = new Error(
      combined || `${command} ${args.join(" ")} failed with exit code ${result.status}`
    );
    error.stdout = stdout;
    error.stderr = stderr;
    throw error;
  }

  return combined;
}

function safeRun(command, args, options = {}) {
  try {
    return run(command, args, options);
  } catch (error) {
    if (typeof error.stdout === "string" && error.stdout) {
      process.stdout.write(error.stdout);
    }
    if (typeof error.stderr === "string" && error.stderr) {
      process.stderr.write(error.stderr);
    }
    throw error;
  }
}

function currentGitSha() {
  return run("git", ["rev-parse", "--short", "HEAD"], { cwd: repoRoot });
}

function timestampTag() {
  const now = new Date();
  const pad = (value) => String(value).padStart(2, "0");
  return [
    now.getUTCFullYear(),
    pad(now.getUTCMonth() + 1),
    pad(now.getUTCDate()),
    "-",
    pad(now.getUTCHours()),
    pad(now.getUTCMinutes()),
    pad(now.getUTCSeconds()),
  ].join("");
}

function extractRegistryImage(output) {
  const match = output.match(/registry\.cloudflare\.com\/[^\s]+/);
  if (!match) {
    throw new Error(`Could not find pushed image URI in Wrangler output:\n${output}`);
  }
  return match[0];
}

function renderDeployConfig(registryImage) {
  const baseConfig = readFileSync(baseConfigPath, "utf8");
  const replaced = baseConfig.replace(
    /"image":\s*"[^"]+"/,
    `"image": "${registryImage}"`
  );

  if (replaced === baseConfig) {
    throw new Error(`Failed to replace container image in ${baseConfigPath}`);
  }

  return replaced;
}

function withTemporaryWranglerDockerfile(runWithDockerfile) {
  if (existsSync(repoDockerfilePath)) {
    throw new Error(
      `Refusing to overwrite ${repoDockerfilePath}. ` +
        "Update deploy.mjs to handle the new canonical Dockerfile first."
    );
  }

  copyFileSync(cloudflareDockerfilePath, repoDockerfilePath);

  try {
    return runWithDockerfile();
  } finally {
    rmSync(repoDockerfilePath, { force: true });
  }
}

function main() {
  const passthroughArgs = process.argv.slice(2);
  const sha = currentGitSha();
  const stamp = timestampTag();
  const localTag = `zitadel-explicit:${sha}-${stamp}`;

  console.log(`Building and pushing immutable image ${localTag} from ${repoRoot}`);
  const buildOutput = withTemporaryWranglerDockerfile(() =>
    safeRun("npx", [
      "wrangler",
      "containers",
      "build",
      "-p",
      "-t",
      localTag,
      repoRoot,
    ])
  );
  if (buildOutput) {
    process.stdout.write(`${buildOutput}\n`);
  }

  const registryImage = extractRegistryImage(buildOutput);
  console.log(`Using immutable registry image ${registryImage}`);

  const tempDir = mkdtempSync(join(tmpdir(), "zitadel-cf-deploy-"));
  const deployConfigPath = join(tempDir, "wrangler.deploy.jsonc");

  try {
    writeFileSync(deployConfigPath, renderDeployConfig(registryImage));

    console.log(`Deploying Worker with explicit image via ${deployConfigPath}`);
    safeRun("npx", [
      "wrangler",
      "deploy",
      "--config",
      deployConfigPath,
      "--containers-rollout=immediate",
      ...passthroughArgs,
    ]);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

main();
