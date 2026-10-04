import assert from "node:assert/strict";
import test from "node:test";
import { createRecordId, createVault, parseVaultEnvelope, unlockVault, VaultError } from "@/lib/vault";

test("vault encrypts a record and decrypts it only while unlocked", async () => {
  const { envelope, session } = await createVault("correct horse battery staple");
  const recordId = createRecordId();
  assert.equal(recordId.includes("credential"), false);
  const stored = await session.sealRecord(envelope, recordId, {
    claim: "membership.active",
    value: true,
  });

  const wire = JSON.stringify(stored);
  assert.equal(wire.includes("membership.active"), false);
  assert.equal(wire.includes('"value":true'), false);

  const opened = await session.openRecord<{ claim: string; value: boolean }>(
    stored,
    recordId,
  );
  assert.deepEqual(opened, { claim: "membership.active", value: true });

  session.lock();
  await assert.rejects(
    () => session.openRecord(stored, recordId),
    (error: unknown) => error instanceof VaultError,
  );
});

test("wrong passphrase and tampered ciphertext fail closed", async () => {
  const { envelope, session } = await createVault("another strong local passphrase");
  const recordId = createRecordId();
  const stored = await session.sealRecord(envelope, recordId, { secret: "fixture" });
  session.lock();

  await assert.rejects(
    () => unlockVault(stored, "not the right passphrase"),
    (error: unknown) => error instanceof VaultError,
  );

  const tampered = structuredClone(stored);
  const original = tampered.records[recordId].ciphertext;
  tampered.records[recordId].ciphertext =
    (original[0] === "A" ? "B" : "A") + original.slice(1);

  const unlocked = await unlockVault(stored, "another strong local passphrase");
  await assert.rejects(
    () => unlocked.openRecord(tampered, recordId),
    (error: unknown) => error instanceof VaultError,
  );
});

test("vault envelope parser rejects weakened KDF parameters", async () => {
  const { envelope, session } = await createVault("third strong local passphrase");
  session.lock();
  const weakened = structuredClone(envelope);
  weakened.kdf.iterations = 10_000;
  assert.throws(
    () => parseVaultEnvelope(weakened),
    (error: unknown) => error instanceof VaultError,
  );
});
