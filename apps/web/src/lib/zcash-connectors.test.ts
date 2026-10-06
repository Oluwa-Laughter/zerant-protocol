import assert from "node:assert/strict";
import test from "node:test";
import { discoverZcashConnectors, injectedConnector, supportsPurpose, zecAuthConnector, zip321Connector, type ZcashConnector } from "./zcash-connectors";
import { directPaymentMode, paymentAction } from "./zcash-payment-connector";
import type { InjectedZcashWalletAdapter, ZecAuthWalletChallenge } from "./zcash-wallet";

function injected(id: string, identitySigning = true): ZcashConnector {
  return injectedConnector({
    id, name: id, capabilities: { identitySigning, shieldedPayment: false, transparentPayment: false,
      paymentRequestHandoff: false, walletConnect: false, zecAuthHandoff: false, connectionRestore: true },
    connect: async () => { throw new Error("not used"); },
    existingConnection: async () => null,
    ensureConnection: async () => { throw new Error("not used"); },
    ...(identitySigning ? { signIdentityChallenge: async () => ({ pubkey: "key", signature: "signature", signingMode: "derived" as const }) } : {}),
  } as InjectedZcashWalletAdapter);
}

const testnet = "zcash:testnet";
const mainnet = "zcash:mainnet";

test("discovery orders detected sign-in wallets and deduplicates transports", () => {
  const choices = discoverZcashConnectors("identity", testnet, "", [zecAuthConnector(), injected("noir"), injected("other"), injected("noir"), zip321Connector()]);
  assert.deepEqual(choices.map((choice) => choice.id), ["noir:injected", "other:injected", "zecauth:portable"]);
});

test("default sign-in discovery does not advertise draft portable ZecAuth", () => {
  const prior = Object.getOwnPropertyDescriptor(globalThis, "window");
  Object.defineProperty(globalThis, "window", { configurable: true, value: {} });
  try {
    const choices = discoverZcashConnectors("identity", testnet, "");
    assert.equal(choices.some((choice) => choice.id === "zecauth:portable"), false);
  } finally {
    if (prior) Object.defineProperty(globalThis, "window", prior);
    else Reflect.deleteProperty(globalThis, "window");
  }
});

test("WalletConnect is not offered by product discovery", () => {
  for (const purpose of ["connection", "payment", "identity"] as const) {
    assert.equal(discoverZcashConnectors(purpose, testnet, "project").some((choice) => choice.transport === "walletconnect"), false);
    assert.equal(discoverZcashConnectors(purpose, mainnet, "").some((choice) => choice.transport === "walletconnect"), false);
    assert.equal(discoverZcashConnectors(purpose, mainnet, "project").some((choice) => choice.transport === "walletconnect"), false);
  }
});

test("identity refuses payment-only and invalid capability advertisements", () => {
  assert.equal(supportsPurpose(zip321Connector(), "identity"), false);
  assert.equal(supportsPurpose(injected("payment", false), "identity"), false);
  const fake = { ...injected("fake"), signIdentityChallenge: undefined };
  assert.deepEqual(discoverZcashConnectors("identity", testnet, "", [fake]), []);
});

test("ZecAuth handoff excludes account and session identifiers", () => {
  const challenge: ZecAuthWalletChallenge = {
    domain: "zerant.example", uri: "https://zerant.example/app", version: 1,
    chain: "zcash:testnet", nonce: "nonce123", issued_at: "2026-10-05T00:00:00Z",
    expiration_time: "2026-10-05T00:05:00Z", statement: "Sign in", scopes: { required: [{ type: "auth" }] },
  };
  const uri = zecAuthConnector().openAuthHandoff!(challenge, "https://zerant.example/api/zerant/auth/verify");
  assert.equal(uri.includes("account_id"), false);
  assert.equal(uri.includes("session_id"), false);
  assert.equal(new URL(uri).searchParams.get("callback"), "https://zerant.example/api/zerant/auth/verify");
});

const simple = { canonical_uri: "zcash:u1recipient?amount=1", payment_count: 1, payments: [{
  index: 0, recipient: "u1recipient", amount_zat: 100_000_000, memo_present: false,
  label: null, message: null, other_param_names: [],
}] };

test("shielded direct is default only for exact simple requests", () => {
  const direct = { ...injected("direct"), capabilities: new Set(["shieldedPayment" as const]), sendShieldedPayment: async () => "tx" };
  assert.deepEqual(paymentAction(direct, simple), { kind: "direct", mode: "shielded", payment: { to: "u1recipient", amount: "1" } });
  assert.deepEqual(paymentAction(zip321Connector(), simple), { kind: "handoff", uri: simple.canonical_uri });
  for (const rich of [
    { ...simple, payment_count: 2, payments: [simple.payments[0], { ...simple.payments[0], index: 1 }] },
    { ...simple, payments: [{ ...simple.payments[0], memo_present: true }] },
    { ...simple, payments: [{ ...simple.payments[0], label: "invoice" }] },
    { ...simple, payments: [{ ...simple.payments[0], message: "invoice" }] },
    { ...simple, payments: [{ ...simple.payments[0], other_param_names: ["foo"] }] },
  ]) {
    assert.equal(directPaymentMode(rich, direct), null);
    assert.throws(() => paymentAction(direct, rich));
    assert.deepEqual(paymentAction(zip321Connector(), rich), { kind: "handoff", uri: rich.canonical_uri });
  }
});

test("transparent direct requires explicit permission", () => {
  const request = { ...simple, payments: [{ ...simple.payments[0], recipient: "t1recipient" }] };
  const direct = { ...injected("transparent"), capabilities: new Set(["transparentPayment" as const]), sendTransparentPayment: async () => "tx" };
  assert.equal(directPaymentMode(request, direct), "transparent");
  assert.throws(() => paymentAction(direct, request));
  assert.deepEqual(paymentAction(direct, request, { allowTransparent: true }), { kind: "direct", mode: "transparent", payment: { to: "t1recipient", amount: "1" } });
});
