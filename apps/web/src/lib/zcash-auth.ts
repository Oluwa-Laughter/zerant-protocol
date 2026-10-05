import type { ZecAuthWalletChallenge } from "./zcash-wallet";
import type { ZcashConnector } from "./zcash-connectors";

export type SignInChallenge = ZecAuthWalletChallenge & { message: string };
export async function signInWithZcashWallet(connector: ZcashConnector, request: typeof fetch = fetch): Promise<void> {
  if (!connector.capabilities.has("identitySigning") || !connector.signIdentityChallenge) {
    throw new Error("This wallet does not support Zerant sign-in. Use a passkey for account access.");
  }
  const response = await request("/api/zerant/auth/challenge?scopes=signin", { credentials: "same-origin", cache: "no-store" });
  if (!response.ok) throw new Error("Zerant could not create a wallet sign-in request.");
  const challenge = (await response.json()) as SignInChallenge;
  const signed = await connector.signIdentityChallenge(challenge.message);
  const verified = await request("/api/zerant/auth/wallet/verify", {
    method: "POST", credentials: "same-origin", headers: { "content-type": "application/json" },
    body: JSON.stringify({ pubkey: signed.pubkey, signature: signed.signature,
      message: challenge.message, granted: ["auth"], signing_mode: signed.signingMode }),
  });
  if (!verified.ok) {
    if (verified.status === 401) {
      throw new Error("Noir approved signing, but the returned signature did not verify against Zerant’s challenge.");
    }
    if (verified.status === 429) {
      throw new Error("Wallet sign-in is being attempted too quickly. Try again in a minute.");
    }
    throw new Error("Noir approved signing, but Zerant could not accept the wallet signature response.");
  }
  const redeemed = await request("/api/zerant/auth/session", { credentials: "same-origin", cache: "no-store" });
  if (!redeemed.ok) {
    throw new Error("Wallet signature verified, but the Zerant session could not be completed. Start sign-in again.");
  }
}
