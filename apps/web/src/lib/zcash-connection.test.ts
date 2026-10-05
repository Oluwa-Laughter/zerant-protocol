import assert from "node:assert/strict";
import test from "node:test";
import { connectConnector, disconnectZcash, getZcashConnectionSnapshot, probeExistingConnection, restoreConnection } from "./zcash-connection";
import { discoverZcashConnectors, injectedConnector, walletConnectConnector } from "./zcash-connectors";
import { registerZcashWalletDetector, type ZcashWalletAdapter } from "./zcash-wallet";
import { signInWithZcashWallet } from "./zcash-auth";
import { directPaymentMode } from "./zcash-payment-connector";
import { WalletConnectZcashAdapter, ZCASH_MAINNET_CAIP } from "./zcash-walletconnect";

function fakeAdapter(id: string, identitySigning = false): ZcashWalletAdapter {
  return { id, name: id, capabilities: { identitySigning, shieldedPayment: false,
    transparentPayment: false, paymentRequestHandoff: false, walletConnect: false,
    zecAuthHandoff: false, connectionRestore: true },
  connect: async () => ({ providerId: id, providerName: id, shieldedAddress: "", transparentAddress: "tm", accountCount: 1 }),
  existingConnection: async () => null,
  ensureConnection: async function () { return this.connect(); },
  disconnect: async () => {},
  ...(identitySigning ? { signIdentityChallenge: async () => ({ pubkey: "key", signature: "sig", signingMode: "derived" as const }) } : {}) };
}

test("registry discovers multiple installed providers and portable paths", async () => {
  const prior = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "window", { configurable: true, value: {} });
  const first = fakeAdapter("first"); const second = fakeAdapter("second");
  const removeFirst = registerZcashWalletDetector(() => first);
  const removeSecond = registerZcashWalletDetector(() => second);
  try {
    const methods = discoverZcashConnectors("connection", "zcash:testnet", "");
    assert.deepEqual(methods.map((method) => method.id), ["first:injected", "second:injected", "zip321:portable"]);
    await connectConnector(methods[1], "zcash:testnet");
    assert.equal(getZcashConnectionSnapshot().account?.providerId, "second");
    assert.equal(getZcashConnectionSnapshot().selected?.id, "second:injected");
    await disconnectZcash();
    assert.equal(getZcashConnectionSnapshot().account, null);
    const restorable = injectedConnector({ ...first, existingConnection: async () => ({ providerId: "first", providerName: "first", shieldedAddress: "", transparentAddress: "tm", accountCount: 1 }) });
    await restoreConnection([restorable], "zcash:testnet");
    assert.equal(getZcashConnectionSnapshot().selected?.id, "first:injected");
    await disconnectZcash();
  } finally { removeFirst(); removeSecond(); if (prior) Object.defineProperty(globalThis, "window", prior); else Reflect.deleteProperty(globalThis, "window"); }
});

test("testnet refuses a detected mainnet wallet before exposing payment actions", async () => {
  let disconnected = false;
  const mainnet = injectedConnector({ ...fakeAdapter("mainnet"), disconnect: async () => { disconnected = true; }, connect: async () => ({
    providerId: "mainnet", providerName: "Mainnet wallet", shieldedAddress: "u1mainnet",
    transparentAddress: "t1mainnet", accountCount: 1,
  }) });
  await assert.rejects(connectConnector(mainnet, "zcash:testnet"), /does not match Zerant’s testnet network/);
  assert.equal(getZcashConnectionSnapshot().account, null);
  assert.equal(getZcashConnectionSnapshot().selected, null);
  assert.equal(disconnected, true);
});

test("payment-only connection cannot authenticate and creates no network request", async () => {
  let calls = 0;
  const request = (async () => { calls++; return new Response(null, { status: 500 }); }) as typeof fetch;
  await assert.rejects(signInWithZcashWallet(injectedConnector(fakeAdapter("payment")), request));
  const misleading = { ...injectedConnector(fakeAdapter("identity", true)), capabilities: new Set<"transparentPayment">() };
  await assert.rejects(signInWithZcashWallet(misleading, request));
  assert.equal(calls, 0);
});

test("identity signing uses challenge and verification, never address or balance", async () => {
  const calls: string[] = [];
  const request = (async (url: string | URL | Request) => {
    calls.push(String(url));
    return String(url).includes("challenge") ? Response.json({ message: "challenge" }) : Response.json({});
  }) as typeof fetch;
  await signInWithZcashWallet(injectedConnector(fakeAdapter("identity", true)), request);
  assert.deepEqual(calls, ["/api/zerant/auth/challenge?scopes=signin", "/api/zerant/auth/wallet/verify", "/api/zerant/auth/session"]);
});

test("rich or shielded requests cannot be downgraded to transparent payment", () => {
  const adapter = fakeAdapter("transparent"); adapter.capabilities.transparentPayment = true;
  adapter.sendTransparentPayment = async () => "tx";
  const base = { canonical_uri: "zcash:u1?amount=1", payment_count: 1, payments: [{ index: 0, recipient: "u1recipient", amount_zat: 100_000_000, memo_present: false, label: null, message: null, other_param_names: [] }] };
  const connector = injectedConnector(adapter);
  assert.equal(directPaymentMode(base, connector), null);
  assert.equal(directPaymentMode({ ...base, payments: [{ ...base.payments[0], recipient: "t1recipient", memo_present: true }] }, connector), null);
  assert.equal(directPaymentMode({ ...base, payments: [{ ...base.payments[0], recipient: "t1recipient" }] }, connector), "transparent");
});

test("WalletConnect gains transparent payment only after a verified transferable transparent session", async () => {
  const requests: unknown[] = []; const displayed: string[] = []; let disconnected = false;
  const session = { topic: "topic", namespaces: { bip122: { accounts: [`${ZCASH_MAINNET_CAIP}:t1recipient`], methods: ["zcash_getAddress", "zcash_transfer"] } } };
  let available = false;
  const adapter = new WalletConnectZcashAdapter("project", (uri) => displayed.push(uri), async () => ({
    session: { getAll: () => available ? [session] : [] },
    connect: async () => ({ uri: "wc:pairing", approval: async () => { available = true; return session; } }),
    disconnect: async () => { disconnected = true; },
    request: async (input: unknown) => { requests.push(input); return "tx"; },
  }));
  const connector = walletConnectConnector("project", (uri) => displayed.push(uri), adapter);
  assert.deepEqual([...connector.capabilities].sort(), ["connectionRestore", "walletConnect"]);
  assert.equal(connector.capabilities.has("identitySigning"), false);
  assert.equal(connector.capabilities.has("shieldedPayment"), false);
  await connector.connect!();
  assert.deepEqual(displayed, ["wc:pairing"]);
  assert.equal(connector.capabilities.has("transparentPayment"), true);
  assert.equal((await connector.existingConnection!())?.providerId, "walletconnect");
  assert.deepEqual(requests, []);
  await connector.disconnect!(); assert.equal(disconnected, true);
  assert.equal(connector.capabilities.has("transparentPayment"), false);
});

test("WalletConnect does not advertise payment without transfer method or transparent account", async () => {
  for (const [address, methods] of [
    ["t1recipient", ["zcash_getAddress"]],
    ["u1recipient", ["zcash_getAddress", "zcash_transfer"]],
  ] as const) {
    const session = { topic: "topic", namespaces: { bip122: {
      accounts: [`${ZCASH_MAINNET_CAIP}:${address}`], methods: [...methods],
    } } };
    const requests: unknown[] = [];
    const adapter = new WalletConnectZcashAdapter("project", () => {}, async () => ({
      session: { getAll: () => [session] },
      connect: async () => { throw new Error("Restored session should be used."); },
      disconnect: async () => {},
      request: async (input: unknown) => { requests.push(input); return "tx"; },
    }));
    const connector = walletConnectConnector("project", () => {}, adapter);
    assert.equal(connector.capabilities.has("transparentPayment"), false);
    assert.ok(await connector.existingConnection!());
    assert.equal(connector.capabilities.has("transparentPayment"), false);
    assert.equal(connector.capabilities.has("identitySigning"), false);
    assert.equal(connector.capabilities.has("shieldedPayment"), false);
    assert.deepEqual(requests, []);
  }
});


test("silent wallet probe distinguishes authorization and network mismatch", async () => {
  const base = injectedConnector({
    ...fakeAdapter("test"),
    existingConnection: async () => ({
      providerId: "test",
      providerName: "Test wallet",
      shieldedAddress: "utest1example",
      transparentAddress: "tmExample",
      accountCount: 1,
    }),
  });
  assert.equal(await probeExistingConnection(base, "zcash:testnet"), "ready");
  assert.equal(await probeExistingConnection(base, "zcash:mainnet"), "wrong_network");
  const pending = injectedConnector({ ...fakeAdapter("pending"), existingConnection: async () => null });
  assert.equal(await probeExistingConnection(pending, "zcash:testnet"), "not_authorized");
});
