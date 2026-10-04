import type { InjectedZcashWalletAdapter } from "./zcash-wallet";

export async function submitZcashLink(
  wallet: InjectedZcashWalletAdapter,
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
