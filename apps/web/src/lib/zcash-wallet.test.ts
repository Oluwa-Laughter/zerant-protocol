import assert from "node:assert/strict";
import test from "node:test";
import type { NoirWalletProvider } from "@noir-wallet/sdk";
import {
  buildZecAuthWalletUri,
  connectedWalletNetwork,
  NoirWalletAdapter,
  normalizeDerivedSignature,
  normalizeWalletConnection,
  zatoshiToZec,
} from "./zcash-wallet";

test("wallet network check rejects mixed and unrecognized addresses", () => {
  const base = { providerId: "wallet", providerName: "Wallet", accountCount: 1 };
  assert.equal(connectedWalletNetwork({ ...base, shieldedAddress: "utest1account", transparentAddress: "tmaccount" }), "zcash:testnet");
  assert.equal(connectedWalletNetwork({ ...base, shieldedAddress: "u1account", transparentAddress: "t1account" }), "zcash:mainnet");
  assert.equal(connectedWalletNetwork({ ...base, shieldedAddress: "utest1account", transparentAddress: "t1account" }), null);
  assert.equal(connectedWalletNetwork({ ...base, shieldedAddress: "unknown", transparentAddress: "" }), null);
});

test("normalizes wallet metadata without hard-coding a provider brand", () => {
  const result = normalizeWalletConnection(
    {
      transparent: "t1-example",
      shielded: "u1-example",
      accounts: [{ id: "one" }, { id: "two" }],
    },
    "example-wallet",
    "Example Wallet",
  );

  assert.equal(result.providerId, "example-wallet");
  assert.equal(result.providerName, "Example Wallet");
  assert.equal(result.shieldedAddress, "u1-example");
  assert.equal(result.accountCount, 2);
});

test("accepts a usable account even when a wallet exposes only one address class", () => {
  const result = normalizeWalletConnection(
    { shielded: "u1-shielded", accounts: [] },
    "shielded-only",
    "Shielded Wallet",
  );

  assert.equal(result.shieldedAddress, "u1-shielded");
  assert.equal(result.transparentAddress, "");
  assert.equal(result.accountCount, 1);
});

test("requires derived signing mode for injected Zerant identity", () => {
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

test("builds a portable ZecAuth wallet-app handoff", () => {
  const uri = buildZecAuthWalletUri(
    {
      domain: "zerant.vercel.app",
      uri: "https://zerant.vercel.app/app",
      version: 1,
      chain: "zcash:testnet",
      nonce: "nonce-1234567890",
      issued_at: "2026-10-04T18:00:00Z",
      expiration_time: "2026-10-04T18:05:00Z",
      statement: "Authenticate to Zerant.",
      scopes: { required: [{ type: "auth" }] },
    },
    "https://zerant.vercel.app/api/zerant/auth/verify",
  );

  assert.ok(uri.startsWith("zecauth://zerant.vercel.app?"));
  const query = uri.split("?")[1];
  const params = new URLSearchParams(query);
  assert.equal(
    params.get("callback"),
    "https://zerant.vercel.app/api/zerant/auth/verify",
  );
  const challenge = JSON.parse(params.get("challenge") ?? "{}") as Record<string, unknown>;
  assert.equal(challenge.chain, "zcash:testnet");
  assert.equal(challenge.domain, "zerant.vercel.app");
  assert.equal("message" in challenge, false);
});

test("rejects invalid wallet-app handoff callbacks", () => {
  assert.throws(() =>
    buildZecAuthWalletUri(
      {
        domain: "zerant.vercel.app",
        uri: "https://zerant.vercel.app/app",
        version: 1,
        chain: "zcash:testnet",
        nonce: "nonce",
        issued_at: "2026-10-04T18:00:00Z",
        expiration_time: "2026-10-04T18:05:00Z",
        statement: "Authenticate to Zerant.",
        scopes: { required: [{ type: "auth" }] },
      },
      "javascript:alert(1)",
    ),
  );
});

test("converts zatoshis to an exact ZEC decimal string", () => {
  assert.equal(zatoshiToZec(1), "0.00000001");
  assert.equal(zatoshiToZec(125_000_000), "1.25");
  assert.equal(zatoshiToZec(100_000_000), "1");
  assert.throws(() => zatoshiToZec(0));
  assert.throws(() => zatoshiToZec(Number.MAX_SAFE_INTEGER + 1));
});

function fakeWallet(overrides: Record<string, unknown> = {}): NoirWalletProvider {
  return {
    isNoirWallet: true,
    zcash: {
      getAccounts: async () => null,
      connect: async () => ({
        transparent: "t1-connected",
        shielded: "u1-connected",
        accounts: [],
      }),
      signMessage: async () => ({
        pubkey: "02".padEnd(66, "1"),
        signature: "1f".padEnd(130, "2"),
        address: "t1-derived",
        signingMode: "derived",
      }),
      sendTransaction: async () => "txid-1",
      disconnect: async () => {},
      ...overrides,
    },
  } as unknown as NoirWalletProvider;
}

test("one injected adapter restores an existing authorization without prompting", async () => {
  let connectCalls = 0;
  const wallet = fakeWallet({
    getAccounts: async () => ({
      transparent: "t1-existing",
      shielded: "u1-existing",
      accounts: [{ id: "existing" }],
    }),
    connect: async () => {
      connectCalls += 1;
      throw new Error("connect should not be called");
    },
  });

  const adapter = new NoirWalletAdapter(wallet);
  const connection = await adapter.existingConnection();
  assert.ok(connection);
  assert.equal(connection.shieldedAddress, "u1-existing");
  assert.equal(connection.providerId, "noir");
  assert.equal(connectCalls, 0);
});

test("an explicit Noir connect opens approval even when silent lookup would reject", async () => {
  let connectCalls = 0;
  let lookupCalls = 0;
  const wallet = fakeWallet({
    getAccounts: async () => { lookupCalls += 1; throw new Error("Not authorized"); },
    connect: async () => {
      connectCalls += 1;
      return {
        transparent: "t1-connected",
        shielded: "u1-connected",
        accounts: [{ id: "new" }],
      };
    },
  });

  const adapter = new NoirWalletAdapter(wallet);
  const connection = await adapter.ensureConnection();
  assert.equal(connection.shieldedAddress, "u1-connected");
  assert.equal(connectCalls, 1);
  assert.equal(lookupCalls, 0);
});

test("enhanced injected adapter advertises and uses supported capabilities", async () => {
  let signingMode: unknown;
  let fundingSource: unknown;
  const wallet = fakeWallet({
    signMessage: async (_message: string, options: { signingMode?: string }) => {
      signingMode = options.signingMode;
      return {
        pubkey: "02".padEnd(66, "1"),
        signature: "1f".padEnd(130, "2"),
        address: "t1-derived",
        signingMode: "derived",
      };
    },
    sendTransaction: async (params: { fundingSource?: string }) => {
      fundingSource = params.fundingSource;
      return "txid-shielded";
    },
  });

  const adapter = new NoirWalletAdapter(wallet);
  assert.equal(adapter.capabilities.identitySigning, true);
  assert.equal(adapter.capabilities.shieldedPayment, true);

  await adapter.signIdentityChallenge("challenge");
  const txid = await adapter.sendShieldedPayment({
    to: "u1-recipient",
    amount: "0.1",
  });

  assert.equal(signingMode, "derived");
  assert.equal(fundingSource, "shielded");
  assert.equal(txid, "txid-shielded");
});
