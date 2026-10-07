"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { removeWalletMessageMethod, removeZecAuthMethod } from "@/lib/zcash-link";

export type LinkedZcashMethod = {
  method: "zecauth" | "wallet_message";
  chain: string | null;
  created_at: string;
};

// Existing access methods remain removable. New account access is passkey-first;
// payment handoff never creates a linked sign-in method.
export function ZcashSignInManager({
  initialMethods,
  onCountChange,
}: {
  initialMethods: LinkedZcashMethod[];
  onCountChange?: (count: number) => void;
}) {
  const [methods, setMethods] = useState(initialMethods);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const [pendingRemoval, setPendingRemoval] = useState<string | null>(null);

  async function removeMethod(method: LinkedZcashMethod) {
    if (busy) return;
    setBusy(true);
    try {
      const response = method.method === "zecauth"
        ? await removeZecAuthMethod()
        : await removeWalletMessageMethod(method.chain ?? "");
      if (!response.ok) {
        setStatus(response.status === 409
          ? "Add a passkey before removing your last account access method."
          : response.status === 401 || response.status === 403
            ? "Sign in again to remove this access method."
            : "This access method could not be removed. Try again.");
        return;
      }
      setPendingRemoval(null);
      const refreshed = await fetch("/api/zerant/account/zcash/methods", {
        credentials: "same-origin", cache: "no-store",
      });
      if (refreshed.ok) {
        const next = (await refreshed.json()) as LinkedZcashMethod[];
        setMethods(next);
        onCountChange?.(next.length);
      }
      setStatus("Zcash sign-in removed. Other sessions using it were signed out.");
    } catch {
      setStatus("This access method could not be removed. Try again.");
    } finally {
      setBusy(false);
    }
  }

  if (!methods.length) return null;

  return (
    <article className="account-card">
      <p className="eyebrow">Previously linked access</p>
      <h2>Older Zcash sign-in methods</h2>
      <p className="muted">These methods can still open your account until you remove them. Payments use a separate wallet handoff and do not link a sign-in method.</p>
      <ul>
        {methods.map((method) => {
          const key = `${method.method}:${method.chain ?? ""}`;
          const label = method.method === "zecauth" ? "ZecAuth" : "Wallet message";
          return <li key={key}>
            {label}{method.chain ? ` · ${method.chain.replace("zcash:", "")}` : ""}
            {pendingRemoval === key ? (
              <div className="account-access-confirm">
                <span className="small muted">Remove this sign-in method?</span>
                <div className="vault-actions wrap">
                  <Button variant="secondary" onClick={() => void removeMethod(method)} disabled={busy}>Confirm removal</Button>
                  <Button variant="secondary" onClick={() => setPendingRemoval(null)} disabled={busy}>Cancel</Button>
                </div>
              </div>
            ) : <Button variant="secondary" onClick={() => setPendingRemoval(key)} disabled={busy} aria-label={`Remove ${label} sign-in`}>Remove</Button>}
          </li>;
        })}
      </ul>
      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </article>
  );
}
