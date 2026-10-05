import { buildZecAuthWalletUri, type ZecAuthWalletChallenge, type WalletMessageSignature } from "./zcash-wallet";

export async function removeZecAuthMethod(request: typeof fetch = fetch): Promise<Response> {
  return request("/api/zerant/account/zcash/zecauth", {
    method: "DELETE", credentials: "same-origin", cache: "no-store",
  });
}

export async function removeWalletMessageMethod(chain: string, request: typeof fetch = fetch): Promise<Response> {
  const network = chain === "zcash:testnet" ? "testnet" : chain === "zcash:mainnet" ? "mainnet" : null;
  if (!network) throw new Error("Unsupported Zcash network.");
  return request(`/api/zerant/account/zcash/wallet/${network}`, {
    method: "DELETE", credentials: "same-origin", cache: "no-store",
  });
}

export async function startWalletAppLink(origin: string, request: typeof fetch = fetch): Promise<string> {
  const response = await request("/api/zerant/account/zcash/challenge", {
    method: "POST", credentials: "same-origin", cache: "no-store",
  });
  if (!response.ok) throw new Error(response.status === 403 ? "Sign in again to link a Zcash wallet." : "Could not start Zcash sign-in linking.");
  const challenge = (await response.json()) as ZecAuthWalletChallenge;
  return buildZecAuthWalletUri(challenge, origin + "/api/zerant/account/zcash/zecauth/callback");
}

export async function completeWalletAppLink(request: typeof fetch = fetch): Promise<Response> {
  return request("/api/zerant/account/zcash/zecauth/complete", {
    method: "POST", credentials: "same-origin", cache: "no-store",
  });
}

export async function submitZcashLink(
  wallet: { signIdentityChallenge?: (message: string) => Promise<WalletMessageSignature> },
  request: typeof fetch = fetch,
): Promise<{ stage: "challenge" | "verify"; response: Response }> {
  if (!wallet.signIdentityChallenge) throw new Error("Wallet cannot sign identity challenges.");
  const challenge = await request("/api/zerant/account/zcash/challenge", {
    method: "POST", credentials: "same-origin", cache: "no-store",
  });
  if (!challenge.ok) return { stage: "challenge", response: challenge };
  const { message } = (await challenge.json()) as { message: string };
  const signed = await wallet.signIdentityChallenge(message);
  const response = await request("/api/zerant/account/zcash/wallet/verify", {
    method: "POST", credentials: "same-origin", cache: "no-store",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      message, pubkey: signed.pubkey, signature: signed.signature,
      signing_mode: signed.signingMode, granted: ["auth"],
    }),
  });
  return { stage: "verify", response };
}
