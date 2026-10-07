#!/usr/bin/env node
// CinaVault Plus license tool. Uses only node:crypto.
//
// Create the signing keypair (once, kept OUTSIDE this repository):
//   node scripts/issue-license.mjs --generate-keypair --out ~/cinavault-license-private.pem
//   -> prints the base64 public key; set it as the CINAVAULT_LICENSE_PUBLIC_KEY
//      build variable so the app can verify licenses.
//
// Issue a license token:
//   node scripts/issue-license.mjs --email buyer@example.com --months 1 --key ~/cinavault-license-private.pem
//   (or set CINAVAULT_LICENSE_PRIVATE_KEY to the PEM text instead of --key)
//   -> prints CVL1.<base64url payload>.<base64url signature>
//
// Token format (must match src-tauri/src/entitlements.rs):
//   payload  = JSON {license_id, email, plan: "cinavault_plus", issued_at, expires_at} (RFC 3339)
//   signed   = ASCII bytes of "CVL1.<base64url(payload)>"
//   token    = signed + "." + base64url(Ed25519 signature)

import { createPrivateKey, generateKeyPairSync, randomUUID, sign } from "node:crypto";
import { existsSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve, isAbsolute } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const TOKEN_PREFIX = "CVL1";
export const PLAN_ID = "cinavault_plus";
const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const USAGE = `Usage:
  issue-license.mjs --generate-keypair --out <private-key-file outside the repo>
  issue-license.mjs --email <email> --months <N> [--key <private-key-file>] [--license-id <id>] [--issued-at <RFC3339>]
The private key comes from --key or the CINAVAULT_LICENSE_PRIVATE_KEY environment variable.`;

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

export function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) throw new Error(`Unexpected argument: ${arg}`);
    const name = arg.slice(2);
    if (name === "generate-keypair" || name === "help") {
      args[name] = true;
      continue;
    }
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) throw new Error(`--${name} needs a value`);
    args[name] = value;
    index += 1;
  }
  return args;
}

const base64url = (buffer) => Buffer.from(buffer).toString("base64url");
const rfc3339 = (date) => date.toISOString().replace(/\.\d{3}Z$/, "Z");

/** Resolve the nearest existing ancestor so symlinks into the repo are caught. */
function realResolve(path) {
  let current = resolve(path);
  const suffix = [];
  while (!existsSync(current)) {
    suffix.unshift(current.slice(dirname(current).length + 1));
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return resolve(realpathSync(current), ...suffix);
}

export function isInsideRepo(path, repoRoot = REPO_ROOT) {
  const rel = relative(realpathSync(repoRoot), realResolve(path));
  return rel === "" || (!rel.startsWith("..") && !isAbsolute(rel));
}

export function generateKeypair(outPath, repoRoot = REPO_ROOT) {
  if (!outPath) throw new Error("--out <file> is required for the private key");
  if (isInsideRepo(outPath, repoRoot)) {
    throw new Error(`Refusing to write the private key inside the repository (${repoRoot}). Choose a path outside it.`);
  }
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const pem = privateKey.export({ type: "pkcs8", format: "pem" });
  // "wx": never overwrite an existing key file.
  writeFileSync(outPath, pem, { mode: 0o600, flag: "wx" });
  return publicKeyBase64(publicKey);
}

export function publicKeyBase64(keyObject) {
  const jwk = keyObject.export({ format: "jwk" });
  return Buffer.from(jwk.x, "base64url").toString("base64");
}

export function loadPrivateKey(keyPath, env = process.env) {
  let material;
  if (keyPath) {
    material = readFileSync(keyPath, "utf8");
  } else if (env.CINAVAULT_LICENSE_PRIVATE_KEY && env.CINAVAULT_LICENSE_PRIVATE_KEY.trim()) {
    material = env.CINAVAULT_LICENSE_PRIVATE_KEY;
  } else {
    throw new Error("No private key: pass --key <file> or set CINAVAULT_LICENSE_PRIVATE_KEY");
  }
  const trimmed = material.trim();
  const key = trimmed.startsWith("-----BEGIN")
    ? createPrivateKey(trimmed)
    : createPrivateKey({ key: Buffer.from(trimmed, "base64"), format: "der", type: "pkcs8" });
  if (key.asymmetricKeyType !== "ed25519") throw new Error("The private key is not an Ed25519 key");
  return key;
}

export function addMonths(date, months) {
  const result = new Date(date.getTime());
  const day = result.getUTCDate();
  result.setUTCDate(1);
  result.setUTCMonth(result.getUTCMonth() + months);
  const lastDay = new Date(Date.UTC(result.getUTCFullYear(), result.getUTCMonth() + 1, 0)).getUTCDate();
  result.setUTCDate(Math.min(day, lastDay));
  return result;
}

export function issueToken({ privateKey, email, months, licenseId = randomUUID(), issuedAt = new Date() }) {
  const cleanEmail = String(email ?? "").trim();
  if (!cleanEmail.includes("@")) throw new Error("--email must be an email address");
  const monthCount = Number(months);
  if (!Number.isInteger(monthCount) || monthCount < 1 || monthCount > 1200) {
    throw new Error("--months must be a whole number from 1 to 1200");
  }
  if (Number.isNaN(issuedAt.getTime())) throw new Error("--issued-at is not a valid time");
  const payload = {
    license_id: String(licenseId),
    email: cleanEmail,
    plan: PLAN_ID,
    issued_at: rfc3339(issuedAt),
    expires_at: rfc3339(addMonths(issuedAt, monthCount)),
  };
  const signed = `${TOKEN_PREFIX}.${base64url(JSON.stringify(payload))}`;
  const signature = sign(null, Buffer.from(signed, "ascii"), privateKey);
  return { token: `${signed}.${base64url(signature)}`, payload };
}

function main(argv) {
  let args;
  try {
    args = parseArgs(argv);
  } catch (error) {
    fail(`${error.message}\n${USAGE}`);
  }
  if (args.help || argv.length === 0) {
    process.stdout.write(`${USAGE}\n`);
    return;
  }
  try {
    if (args["generate-keypair"]) {
      const publicKey = generateKeypair(args.out);
      process.stderr.write(`Private key written to ${resolve(args.out)} (keep it secret, never commit it).\n`);
      process.stderr.write("Public key (set as CINAVAULT_LICENSE_PUBLIC_KEY):\n");
      process.stdout.write(`${publicKey}\n`);
      return;
    }
    const privateKey = loadPrivateKey(args.key);
    const { token } = issueToken({
      privateKey,
      email: args.email,
      months: args.months,
      licenseId: args["license-id"],
      issuedAt: args["issued-at"] ? new Date(args["issued-at"]) : new Date(),
    });
    process.stdout.write(`${token}\n`);
  } catch (error) {
    fail(error.message);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main(process.argv.slice(2));
}
