import assert from "node:assert/strict";
import test from "node:test";
import type { InjectedZcashWalletAdapter } from "./zcash-wallet";
import { submitZcashLink } from "./zcash-link";

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
