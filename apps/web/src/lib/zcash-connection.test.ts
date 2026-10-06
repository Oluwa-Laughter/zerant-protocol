import assert from "node:assert/strict";
import test from "node:test";
import { connectConnector, disconnectZcash, getZcashConnectionSnapshot, probeExistingConnection, resetConnectorAuthorization, restoreConnection } from "./zcash-connection";
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

test("fresh wallet discovery never asks for accounts before the selected wallet connects", async () => {
  const prior = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "window", { configurable: true, value: {} });
  let lookupCalls = 0;
  let connectCalls = 0;
  const adapter: ZcashWalletAdapter = {
    ...fakeAdapter("passive-noir"),
    existingConnection: async () => { lookupCalls += 1; return null; },
    connect: async () => {
      connectCalls += 1;
      return { providerId: "passive-noir", providerName: "Testnet wallet", shieldedAddress: "utest1connected", transparentAddress: "tmConnected", accountCount: 1 };
    },
  };
  const remove = registerZcashWalletDetector(() => adapter);
  try {
    const choices = discoverZcashConnectors("connection", "zcash:testnet");
    assert.equal(lookupCalls, 0);
    assert.equal(connectCalls, 0);
    await connectConnector(choices[0], "zcash:testnet");
    assert.equal(connectCalls, 1);
    assert.equal(lookupCalls, 0);
    await disconnectZcash();
  } finally {
    remove();
    if (prior) Object.defineProperty(globalThis, "window", prior);
    else Reflect.deleteProperty(globalThis, "window");
  }
});

test("rejected interactive connection keeps the wallet result and does not run recovery", async () => {
  const rejected = { code: 4001, message: "User rejected" };
  let lookupCalls = 0;
  let disconnectCalls = 0;
  const connector = injectedConnector({
    ...fakeAdapter("rejected-noir"),
    connect: async () => { throw rejected; },
    existingConnection: async () => { lookupCalls += 1; return null; },
    disconnect: async () => { disconnectCalls += 1; },
  });
  await assert.rejects(connectConnector(connector, "zcash:testnet"), (error) => error === rejected);
  assert.equal(lookupCalls, 0);
  assert.equal(disconnectCalls, 0);
  assert.equal(getZcashConnectionSnapshot().account, null);
  assert.equal(getZcashConnectionSnapshot().status, "idle");
});

test("a new approval detaches the previous wallet's account events first", async () => {
  let oldEvent: (() => void) | null = null;
  let cleanupCalls = 0;
  let oldLookupCalls = 0;
  const previous = injectedConnector({
    ...fakeAdapter("previous"),
    connect: async () => ({ providerId: "previous", providerName: "Previous", shieldedAddress: "utest1previous", transparentAddress: "tmPrevious", accountCount: 1 }),
    existingConnection: async () => { oldLookupCalls += 1; return null; },
    subscribeConnectionChanges: (handler) => {
      oldEvent = handler;
      return () => { cleanupCalls += 1; oldEvent = null; };
    },
  });
  await connectConnector(previous, "zcash:testnet");
  const next = injectedConnector({
    ...fakeAdapter("next"),
    connect: async () => {
      assert.equal(cleanupCalls, 1);
      (oldEvent as (() => void) | null)?.();
      return { providerId: "next", providerName: "Next", shieldedAddress: "utest1next", transparentAddress: "tmNext", accountCount: 1 };
    },
  });
  await connectConnector(next, "zcash:testnet");
  assert.equal(oldLookupCalls, 0);
  assert.equal(getZcashConnectionSnapshot().account?.providerId, "next");
  await disconnectZcash();
});

test("an old account event cannot overwrite a new interactive connection", async () => {
  let oldEvent: (() => void) | null = null;
  let finishLookup: (account: Awaited<ReturnType<NonNullable<ZcashWalletAdapter["existingConnection"]>>>) => void = () => {
    throw new Error("Account lookup did not start.");
  };
  const previous = injectedConnector({
    ...fakeAdapter("previous-pending"),
    connect: async () => ({ providerId: "previous-pending", providerName: "Previous", shieldedAddress: "utest1previous", transparentAddress: "tmPrevious", accountCount: 1 }),
    existingConnection: () => new Promise((resolve) => { finishLookup = resolve; }),
    subscribeConnectionChanges: (handler) => { oldEvent = handler; return () => { oldEvent = null; }; },
  });
  await connectConnector(previous, "zcash:testnet");
  (oldEvent as (() => void) | null)?.();
  const next = injectedConnector({
    ...fakeAdapter("next-pending"),
    connect: async () => ({ providerId: "next-pending", providerName: "Next", shieldedAddress: "utest1next", transparentAddress: "tmNext", accountCount: 1 }),
  });
  await connectConnector(next, "zcash:testnet");
  finishLookup({ providerId: "previous-pending", providerName: "Previous", shieldedAddress: "u1wrongnetwork", transparentAddress: "t1Wrong", accountCount: 1 });
  await Promise.resolve();
  assert.equal(getZcashConnectionSnapshot().account?.providerId, "next-pending");
  assert.equal(getZcashConnectionSnapshot().status, "connected");
  await disconnectZcash();
});

test("identity restore reuses existing wallet authorization without prompting", async () => {
  let connectCalls = 0;
  let existingCalls = 0;
  const adapter = {
    ...fakeAdapter("identity-restore", true),
    connect: async () => {
      connectCalls += 1;
      return { providerId: "identity-restore", providerName: "Identity wallet", shieldedAddress: "utest1restored", transparentAddress: "tmrestored", accountCount: 1 };
    },
    existingConnection: async () => {
      existingCalls += 1;
      return { providerId: "identity-restore", providerName: "Identity wallet", shieldedAddress: "utest1restored", transparentAddress: "tmrestored", accountCount: 1 };
    },
  };
  const restored = await restoreConnection([injectedConnector(adapter)], "zcash:testnet");
  assert.equal(restored?.providerId, "identity-restore");
  assert.equal(existingCalls, 1);
  assert.equal(connectCalls, 0);
  assert.equal(getZcashConnectionSnapshot().selected?.capabilities.has("identitySigning"), true);
  await disconnectZcash();
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
  const stages: string[] = [];
  const request = (async (url: string | URL | Request) => {
    calls.push(String(url));
    return String(url).includes("challenge") ? Response.json({ message: "challenge" }) : Response.json({});
  }) as typeof fetch;
  await signInWithZcashWallet(
    injectedConnector(fakeAdapter("identity", true)),
    request,
    (stage) => stages.push(stage),
  );
  assert.deepEqual(calls, ["/api/zerant/auth/challenge?scopes=signin", "/api/zerant/auth/wallet/verify", "/api/zerant/auth/session"]);
  assert.deepEqual(stages, ["challenge", "wallet_approval", "verification", "session"]);
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


test("authorization reset clears selected state and verifies Noir permission is gone", async () => {
  let authorized = true;
  let disconnectCalls = 0;
  const adapter: ZcashWalletAdapter = {
    ...fakeAdapter("reset", true),
    connect: async () => ({
      providerId: "reset", providerName: "Testnet Noir", shieldedAddress: "utest1reset", transparentAddress: "tmReset", accountCount: 1,
    }),
    existingConnection: async () => authorized ? ({
      providerId: "reset", providerName: "Testnet Noir", shieldedAddress: "utest1reset", transparentAddress: "tmReset", accountCount: 1,
    }) : null,
    disconnect: async () => { disconnectCalls += 1; authorized = false; },
  };
  const connector = injectedConnector(adapter);
  await connectConnector(connector, "zcash:testnet");
  assert.equal(getZcashConnectionSnapshot().selected?.id, connector.id);
  assert.equal(await resetConnectorAuthorization(connector, "zcash:testnet"), "not_authorized");
  assert.equal(disconnectCalls, 1);
  assert.equal(getZcashConnectionSnapshot().selected, null);
  assert.equal(getZcashConnectionSnapshot().account, null);
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
  const rejectedLookup = injectedConnector({ ...fakeAdapter("rejected-lookup"), existingConnection: async () => { throw { code: 4100, message: "Unauthorized" }; } });
  assert.equal(await probeExistingConnection(rejectedLookup, "zcash:testnet"), "not_authorized");
  const brokenLookup = injectedConnector({ ...fakeAdapter("broken-lookup"), existingConnection: async () => { throw new Error("Provider crashed"); } });
  assert.equal(await probeExistingConnection(brokenLookup, "zcash:testnet"), "unavailable");
});


test("wallet events refresh same-network accounts and clear stale network state", async () => {
  let onChange: (() => void) | null = null;
  let current = { providerId: "events", providerName: "Testnet Noir", shieldedAddress: "utest1first", transparentAddress: "tmFirst", accountCount: 1 };
  const adapter: ZcashWalletAdapter = {
    ...fakeAdapter("events"),
    connect: async () => current,
    existingConnection: async () => current,
    subscribeConnectionChanges: (handler) => { onChange = handler; return () => { onChange = null; }; },
  };
  const selected = injectedConnector(adapter);
  await connectConnector(selected, "zcash:testnet");
  assert.equal(getZcashConnectionSnapshot().account?.shieldedAddress, "utest1first");

  current = { ...current, shieldedAddress: "utest1second", transparentAddress: "tmSecond" };
  (onChange as (() => void) | null)?.();
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(getZcashConnectionSnapshot().account?.shieldedAddress, "utest1second");

  current = { ...current, shieldedAddress: "u1mainnet", transparentAddress: "t1mainnet" };
  (onChange as (() => void) | null)?.();
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(getZcashConnectionSnapshot().account, null);
  assert.equal(getZcashConnectionSnapshot().selected, null);
});

test("provider disconnect clears wallet authority without affecting Zerant sign-in", async () => {
  let onChange: (() => void) | null = null;
  let authorized = true;
  const account = { providerId: "noir-event", providerName: "Testnet Noir", shieldedAddress: "utest1event", transparentAddress: "tmEvent", accountCount: 1 };
  const adapter: ZcashWalletAdapter = {
    ...fakeAdapter("noir-event"),
    connect: async () => account,
    existingConnection: async () => authorized ? account : null,
    subscribeConnectionChanges: (handler) => { onChange = handler; return () => { onChange = null; }; },
  };
  await connectConnector(injectedConnector(adapter), "zcash:testnet");
  authorized = false;
  (onChange as (() => void) | null)?.();
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(getZcashConnectionSnapshot().account, null);
  assert.equal(getZcashConnectionSnapshot().selected, null);
  assert.equal(getZcashConnectionSnapshot().status, "idle");
});
