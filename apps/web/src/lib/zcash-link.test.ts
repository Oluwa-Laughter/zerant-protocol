import assert from "node:assert/strict";
import test from "node:test";
import type { InjectedZcashWalletAdapter } from "./zcash-wallet";
import { completeWalletAppLink, startWalletAppLink, submitZcashLink } from "./zcash-link";

test("wallet-app link uses the account challenge and dedicated callback without account identifiers", async () => {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = [];
  const request = (async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(url), init });
    return Response.json({
      domain: "zerant.example", uri: "https://zerant.example/app", version: 1,
      chain: "zcash:testnet", nonce: "nonce", issued_at: "start", expiration_time: "end",
      statement: "Link Zcash sign-in", scopes: { required: [{ type: "auth" }] },
      message: "signed-message",
    });
  }) as typeof fetch;
  const uri = new URL(await startWalletAppLink("https://zerant.example", request));
  assert.equal(uri.searchParams.get("callback"), "https://zerant.example/api/zerant/account/zcash/zecauth/callback");
  assert.deepEqual(calls.map(({ url, init }) => [url, init?.method]), [["/api/zerant/account/zcash/challenge", "POST"]]);
  assert.equal(calls[0].init?.body, undefined);
  assert.equal(uri.toString().includes("account_id"), false);
  assert.equal(uri.toString().includes("session_id"), false);
  calls.length = 0;
  await completeWalletAppLink(request);
  assert.deepEqual(calls.map(({ url, init }) => [url, init?.method]), [["/api/zerant/account/zcash/zecauth/complete", "POST"]]);
  assert.equal(calls[0].init?.body, undefined);
});

test("wallet linking uses the account challenge and verify routes without an account id", async () => {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = [];
  let signedMessage = "";
  const wallet = {
    signIdentityChallenge: async (message: string) => {
      signedMessage = message;
      return { pubkey: "public-key", signature: "signature", signingMode: "derived" as const };
    },
  } as InjectedZcashWalletAdapter;
  const request = (async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(url), init });
    return calls.length === 1
      ? Response.json({ message: "server-bound-challenge" })
      : Response.json({ linked: true });
  }) as typeof fetch;

  const result = await submitZcashLink(wallet, request);
  assert.equal(result.stage, "verify");
  assert.equal(signedMessage, "server-bound-challenge");
  assert.deepEqual(calls.map((call) => [call.url, call.init?.method]), [
    ["/api/zerant/account/zcash/challenge", "POST"],
    ["/api/zerant/account/zcash/wallet/verify", "POST"],
  ]);
  assert.equal(calls[0].init?.body, undefined);
  assert.deepEqual(JSON.parse(String(calls[1].init?.body)), {
    message: "server-bound-challenge", pubkey: "public-key", signature: "signature",
    signing_mode: "derived", granted: ["auth"],
  });
  assert.equal(JSON.stringify(calls).includes("account_id"), false);
});
