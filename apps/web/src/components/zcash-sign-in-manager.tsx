"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { getPreferredInjectedZcashWallet } from "@/lib/zcash-wallet";
import { submitZcashLink } from "@/lib/zcash-link";

export type LinkedZcashMethod = {
  method: "zecauth" | "wallet_message";
  chain: string | null;
  created_at: string;
};

export function ZcashSignInManager({ initialMethods }: { initialMethods: LinkedZcashMethod[] }) {
  const [methods, setMethods] = useState(initialMethods);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);

  async function linkWallet() {
    if (busy) return;
    setBusy(true);
    try {
      const wallet = getPreferredInjectedZcashWallet("identitySigning");
      if (!wallet?.signIdentityChallenge) {
        setStatus("No compatible installed Zcash wallet was found.");
        return;
      }
      setStatus("Approve the Zcash sign-in request in your wallet.");
      const result = await submitZcashLink(wallet);
      if (result.stage === "challenge") {
        setStatus(result.response.status === 403 ? "Sign in again to link a Zcash wallet." : "Could not start Zcash sign-in linking.");
        return;
      }
      const verified = result.response;
      if (!verified.ok) {
        setStatus(verified.status === 409
          ? "This Zcash sign-in is already linked elsewhere, or this account has a different wallet sign-in."
          : verified.status === 401 || verified.status === 403
            ? "This request expired or your recent sign-in ended. Sign in again and retry."
            : "Could not link this Zcash sign-in.");
        return;
      }
      const refreshed = await fetch("/api/zerant/account/zcash/methods", {
        credentials: "same-origin", cache: "no-store",
      });
      if (refreshed.ok) setMethods((await refreshed.json()) as LinkedZcashMethod[]);
      setStatus("Zcash sign-in linked to this Zerant account.");
    } catch {
      setStatus("Wallet approval was not completed.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <article className="account-card">
      <p className="eyebrow">Zcash sign-in</p>
      <h2>Linked sign-in methods</h2>
      {methods.length ? (
        <ul>
          {methods.map((method) => (
            <li key={`${method.method}:${method.chain ?? ""}`}>
              {method.method === "zecauth" ? "Wallet app" : "Browser wallet"}
              {method.chain ? ` · ${method.chain.replace("zcash:", "")}` : ""}
              {` · added ${new Date(method.created_at).toLocaleDateString()}`}
            </li>
          ))}
        </ul>
      ) : <p className="muted">No Zcash sign-in method is linked yet.</p>}
      <p className="muted">Linking lets you sign in to this same Zerant account. It does not connect payment information.</p>
      <Button onClick={linkWallet} disabled={busy}>
        {busy ? "Waiting for wallet…" : "Link installed wallet"}
      </Button>
      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </article>
  );
}
