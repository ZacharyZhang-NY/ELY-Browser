#!/usr/bin/env node
import { spawn } from "node:child_process";
import { readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import {
  parsePluginRegistryDocument,
  pluginRegistryKvKey,
} from "../dist/src/plugin_registry.js";
import {
  parseReleaseManifestDocument,
  releaseManifestKvKey,
} from "../dist/src/release_manifests.js";
import {
  parsePublicSigningKeysDocument,
  publicSigningKeysKvKey,
} from "../dist/src/signing_keys.js";

const DOCUMENTS = [
  {
    flag: "--signing-keys",
    label: "public signing keys",
    keyFor: publicSigningKeysKvKey,
    parse: parsePublicSigningKeysDocument,
  },
  {
    flag: "--plugin-registry",
    label: "plugin registry",
    keyFor: pluginRegistryKvKey,
    parse: parsePluginRegistryDocument,
  },
  {
    flag: "--release-manifest",
    label: "release manifest",
    keyFor: releaseManifestKvKey,
    parse: parseReleaseManifestDocument,
  },
];

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const publications = DOCUMENTS.flatMap((document) => {
    const path = options.documents.get(document.flag);
    return path === undefined ? [] : [{ ...document, path }];
  });

  if (publications.length === 0) {
    throw new Error("Provide at least one public cache document path.");
  }

  for (const publication of publications) {
    await publishDocument(options.environment, publication);
  }
}

function parseArgs(args) {
  const options = {
    environment: undefined,
    documents: new Map(),
  };
  const flags = new Set(DOCUMENTS.map((document) => document.flag));

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--help" || arg === "-h") {
      printUsage();
      process.exit(0);
    }
    if (arg === "--environment") {
      options.environment = requiredValue(args, index, arg);
      index += 1;
      continue;
    }
    if (flags.has(arg)) {
      options.documents.set(arg, requiredValue(args, index, arg));
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  if (options.environment === undefined) {
    throw new Error("Missing required --environment value.");
  }

  return options;
}

function requiredValue(args, index, flag) {
  const value = args[index + 1];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}.`);
  }
  return value;
}

async function publishDocument(environment, publication) {
  const sourcePath = resolve(publication.path);
  const raw = await readFile(sourcePath, "utf8");
  const document = publication.parse(raw);
  const kvKey = publication.keyFor(environment);
  const tempPath = join(tmpdir(), `ely-public-cache-${process.pid}-${Date.now()}.json`);
  const normalized = `${JSON.stringify(document, null, 2)}\n`;

  await writeFile(tempPath, normalized, { mode: 0o600 });
  try {
    await runWrangler([
      "kv",
      "key",
      "put",
      kvKey,
      "--binding",
      "ELY_KV",
      "--remote",
      "--path",
      tempPath,
    ]);
    console.log(`Published ${publication.label} to ${kvKey}`);
  } finally {
    await rm(tempPath, { force: true });
  }
}

function runWrangler(args) {
  return new Promise((resolveProcess, rejectProcess) => {
    const command = process.platform === "win32" ? "npx.cmd" : "npx";
    const child = spawn(command, ["wrangler", ...args], {
      stdio: "inherit",
    });

    child.on("error", rejectProcess);
    child.on("exit", (code) => {
      if (code === 0) {
        resolveProcess();
        return;
      }
      rejectProcess(new Error(`wrangler exited with code ${code}`));
    });
  });
}

function printUsage() {
  console.log(`Usage:
  npm run public-cache:publish -- --environment production \\
    --signing-keys ./secure/signing-keys.json \\
    --plugin-registry ./secure/plugin-registry.json \\
    --release-manifest ./secure/release-manifest.json`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
