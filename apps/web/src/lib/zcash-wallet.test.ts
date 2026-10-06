import assert from "node:assert/strict";
import test from "node:test";
import type { NoirWalletProvider } from "@noir-wallet/sdk";
import {
  buildZecAuthWalletUri,
  classifyInjectedWalletError,
  connectedWalletNetwork,
  NoirWalletAdapter,
  normalizeDerivedSignature,
  normalizeTransactionId,
  normalizeWalletConnection,
  zatoshiToZec,
} from "./zcash-wallet";

test("classifies only standard injected wallet request failures", () => {
  assert.equal(classifyInjectedWalletError({ code: 4001, message: "User rejected the request" }), "rejected");
  assert.equal(classifyInjectedWalletError({ code: 4100, message: "Unauthorized" }), "unauthorized");
  assert.equal(classifyInjectedWalletError({ code: -32002, message: "Request already pending" }), "pending");
  assert.equal(classifyInjectedWalletError(new Error("Not authorized")), "unauthorized");
  assert.equal(classifyInjectedWalletError(new Error("Unexpected provider failure")), "unknown");
});

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

test("rejects malformed derived signature encodings before server verification", () => {
  const validPubkey = "02".padEnd(66, "1");
  const validSignature = "1f".padEnd(130, "2");

  assert.throws(() => normalizeDerivedSignature({
    pubkey: "not-hex",
    signature: validSignature,
    signingMode: "derived",
  }), /public key format/);

  assert.throws(() => normalizeDerivedSignature({
    pubkey: validPubkey,
    signature: "abcd",
    signingMode: "derived",
  }), /signature format/);

  assert.throws(() => normalizeDerivedSignature({
    pubkey: validPubkey,
    signature: "1a" + "2".repeat(128),
    signingMode: "derived",
  }), /signature header/);

  assert.equal(
    normalizeDerivedSignature({
      pubkey: "0x" + validPubkey,
      signature: "0x" + validSignature,
      signingMode: "derived",
    }).signingMode,
    "derived",
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

test("accepts only canonical 64-hex Zcash transaction ids", () => {
  const upper = "A".repeat(64);
  assert.equal(normalizeTransactionId(upper), "a".repeat(64));
  assert.throws(() => normalizeTransactionId("txid-shielded"), /invalid Zcash transaction id/);
  assert.throws(() => normalizeTransactionId("0x" + "a".repeat(64)), /invalid Zcash transaction id/);
  assert.throws(() => normalizeTransactionId(""), /did not return a transaction id/);
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

test("Noir connection subscriptions include explicit provider disconnects", () => {
  const added: string[] = [];
  const removed: string[] = [];
  const wallet = fakeWallet({
    on: (event: string) => { added.push(event); },
    removeListener: (event: string) => { removed.push(event); },
  });

  const adapter = new NoirWalletAdapter(wallet);
  const cleanup = adapter.subscribeConnectionChanges(() => undefined);

  assert.deepEqual(added, ["accountsChanged", "chainChanged", "disconnect"]);
  cleanup();
  assert.deepEqual(removed, ["accountsChanged", "chainChanged", "disconnect"]);
});

test("enhanced injected adapter advertises and uses supported capabilities", async () => {
  let signingMode: unknown;
  let fundingSource: unknown;
  let amount: unknown;
  let recipient: unknown;
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
    sendTransaction: async (params: { fundingSource?: string; amount?: string; to?: string }) => {
      fundingSource = params.fundingSource;
      amount = params.amount;
      recipient = params.to;
      return "a".repeat(64);
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
  assert.equal(amount, "0.1");
  assert.equal(recipient, "u1-recipient");
  assert.equal(txid, "a".repeat(64));
});
