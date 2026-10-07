import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createPrivateKey, createPublicKey, verify } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { addMonths, isInsideRepo, publicKeyBase64 } from "../scripts/issue-license.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const script = join(repoRoot, "scripts", "issue-license.mjs");

function run(args, env = {}) {
  const cleanEnv = { ...process.env, ...env };
  if (!("CINAVAULT_LICENSE_PRIVATE_KEY" in env)) delete cleanEnv.CINAVAULT_LICENSE_PRIVATE_KEY;
  return spawnSync(process.execPath, [script, ...args], { encoding: "utf8", env: cleanEnv });
}

function decodeToken(token) {
  const parts = token.split(".");
  assert.equal(parts.length, 3);
  assert.equal(parts[0], "CVL1");
  return {
    signed: `${parts[0]}.${parts[1]}`,
    payload: JSON.parse(Buffer.from(parts[1], "base64url").toString("utf8")),
    signature: Buffer.from(parts[2], "base64url"),
  };
}

test("generates a keypair outside the repo and issues a verifiable token", () => {
  const dir = mkdtempSync(join(tmpdir(), "cinavault-license-"));
  try {
    const keyPath = join(dir, "private.pem");
    const generated = run(["--generate-keypair", "--out", keyPath]);
    assert.equal(generated.status, 0, generated.stderr);
    const publicKey = generated.stdout.trim();
    assert.equal(Buffer.from(publicKey, "base64").length, 32);
    // POSIX permission bits only; Windows reports 0o666 and protects the file with user-profile ACLs.
    if (process.platform !== "win32") {
      assert.equal(statSync(keyPath).mode & 0o077, 0, "private key must not be group/world readable");
    }
    const privateKey = createPrivateKey(readFileSync(keyPath, "utf8"));
    assert.equal(publicKeyBase64(createPublicKey(privateKey)), publicKey);

    // Refuses to overwrite an existing key.
    assert.notEqual(run(["--generate-keypair", "--out", keyPath]).status, 0);

    const issued = run([
      "--email", "buyer@example.com",
      "--months", "3",
      "--key", keyPath,
      "--license-id", "lic_test",
      "--issued-at", "2026-01-31T12:00:00Z",
    ]);
    assert.equal(issued.status, 0, issued.stderr);
    const { signed, payload, signature } = decodeToken(issued.stdout.trim());
    assert.deepEqual(payload, {
      license_id: "lic_test",
      email: "buyer@example.com",
      plan: "cinavault_plus",
      issued_at: "2026-01-31T12:00:00Z",
      expires_at: "2026-04-30T12:00:00Z",
    });
    // The signature covers exactly the ASCII bytes "CVL1.<payload>".
    const rawPublic = createPublicKey({
      key: { kty: "OKP", crv: "Ed25519", x: Buffer.from(publicKey, "base64").toString("base64url") },
      format: "jwk",
    });
    assert.ok(verify(null, Buffer.from(signed, "ascii"), rawPublic, signature));

    // The private key can also come from the environment.
    const fromEnv = run(["--email", "env@example.com", "--months", "1"], {
      CINAVAULT_LICENSE_PRIVATE_KEY: readFileSync(keyPath, "utf8"),
    });
    assert.equal(fromEnv.status, 0, fromEnv.stderr);
    assert.equal(decodeToken(fromEnv.stdout.trim()).payload.email, "env@example.com");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("refuses to write a private key inside the repository", () => {
  const inside = join(repoRoot, "scripts", "should-not-exist.pem");
  assert.ok(isInsideRepo(inside));
  const result = run(["--generate-keypair", "--out", inside]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /inside the repository/);
  assert.throws(() => statSync(inside));
});

test("never falls back to a default key path", () => {
  const result = run(["--email", "a@example.com", "--months", "1"]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /No private key/);
});

test("rejects bad input", () => {
  const dir = mkdtempSync(join(tmpdir(), "cinavault-license-"));
  try {
    const keyPath = join(dir, "k.pem");
    assert.equal(run(["--generate-keypair", "--out", keyPath]).status, 0);
    assert.notEqual(run(["--email", "nope", "--months", "1", "--key", keyPath]).status, 0);
    assert.notEqual(run(["--email", "a@example.com", "--months", "0", "--key", keyPath]).status, 0);
    assert.notEqual(run(["--email", "a@example.com", "--months", "1.5", "--key", keyPath]).status, 0);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("month arithmetic clamps to the end of shorter months", () => {
  assert.equal(addMonths(new Date("2026-01-31T00:00:00Z"), 1).toISOString(), "2026-02-28T00:00:00.000Z");
  assert.equal(addMonths(new Date("2026-03-15T00:00:00Z"), 12).toISOString(), "2027-03-15T00:00:00.000Z");
});
