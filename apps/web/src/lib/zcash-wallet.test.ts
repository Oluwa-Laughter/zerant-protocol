import assert from "node:assert/strict";
import test from "node:test";
import {
  normalizeDerivedSignature,
  normalizeWalletConnection,
  zatoshiToZec,
} from "./zcash-wallet";

test("normalizes a shielded-first injected wallet connection", () => {
  const result = normalizeWalletConnection({
    transparent: "t1-example",
    shielded: "u1-example",
    accounts: [{ id: "one" }, { id: "two" }],
  });

  assert.equal(result.providerName, "Noir Wallet");
  assert.equal(result.shieldedAddress, "u1-example");
  assert.equal(result.accountCount, 2);
});

test("requires derived signing mode for Zerant identity", () => {
  assert.deepEqual(
    normalizeDerivedSignature({
      pubkey: "02".padEnd(66, "1"),
      signature: "1f".padEnd(130, "2"),
      signingMode: "derived",
    }),
    {
      pubkey: "02".padEnd(66, "1"),
      signature: "1f".padEnd(130, "2"),
      signingMode: "derived",
    },
  );

  assert.throws(() =>
    normalizeDerivedSignature({
      pubkey: "pub",
      signature: "sig",
      signingMode: "current",
    }),
  );
});


test("converts zatoshis to an exact ZEC decimal string", () => {
  assert.equal(zatoshiToZec(1), "0.00000001");
  assert.equal(zatoshiToZec(125_000_000), "1.25");
  assert.equal(zatoshiToZec(100_000_000), "1");
  assert.throws(() => zatoshiToZec(0));
  assert.throws(() => zatoshiToZec(Number.MAX_SAFE_INTEGER + 1));
});
