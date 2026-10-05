"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { ZcashWalletSelector } from "@/components/zcash-wallet-selector";
import { type ZcashConnector } from "@/lib/zcash-connectors";
import { completeWalletAppLink, removeWalletMessageMethod, removeZecAuthMethod, startWalletAppLink, submitZcashLink } from "@/lib/zcash-link";

export type LinkedZcashMethod = {
  method: "zecauth" | "wallet_message";
  chain: string | null;
  created_at: string;
};

export function ZcashSignInManager({ initialMethods }: { initialMethods: LinkedZcashMethod[] }) {
  const [methods, setMethods] = useState(initialMethods);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);

  async function refreshMethods() {
    const refreshed = await fetch("/api/zerant/account/zcash/methods", {
      credentials: "same-origin", cache: "no-store",
    });
    if (refreshed.ok) setMethods((await refreshed.json()) as LinkedZcashMethod[]);
    return refreshed.status;
  }

  async function removeMethod(method: LinkedZcashMethod) {
    if (busy) return;
    setBusy(true);
    try {
      const response = method.method === "zecauth"
        ? await removeZecAuthMethod()
        : await removeWalletMessageMethod(method.chain ?? "");
      if (response.ok) {
        setMethods((current) => current.filter((item) =>
          item.method !== method.method || item.chain !== method.chain));
        const refreshStatus = await refreshMethods();
        setStatus(refreshStatus === 401
          ? "Removed. Sign in again with another method."
          : "Zcash sign-in removed. Other Zcash-authenticated sessions were signed out.");
      } else {
        setStatus(response.status === 409
          ? "Add a passkey or another sign-in method before removing your last access method."
          : response.status === 404
            ? "That sign-in method is no longer linked. Refresh this page."
            : response.status === 401 || response.status === 403
              ? "Sign in again to remove a Zcash sign-in method."
              : "Could not remove this Zcash sign-in method.");
      }
    } catch {
      setStatus("Could not remove this Zcash sign-in method.");
    } finally {
      setBusy(false);
    }
  }

  async function linkWalletApp() {
    if (busy) return;
    setBusy(true);
    try {
      const uri = await startWalletAppLink(window.location.origin);
      setHandoffUri(uri);
      setStatus("Open this request in a ZecAuth-compatible wallet, then return here to check approval.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Could not open the Zcash wallet app.");
    } finally {
      setBusy(false);
    }
  }

  async function checkWalletAppApproval() {
    if (busy) return;
    setBusy(true);
    try {
      const response = await completeWalletAppLink();
      if (response.status === 202) {
        setStatus("No wallet app approval is ready yet. Approve it in your wallet, then check again.");
      } else if (response.ok) {
        await refreshMethods();
        setStatus("Zcash sign-in linked to this Zerant account.");
      } else {
        setStatus(response.status === 409
          ? "This Zcash sign-in is already linked elsewhere, or this account has a different wallet sign-in."
          : "This request expired or your recent sign-in ended. Sign in again and retry.");
      }
    } catch {
      setStatus("Could not check wallet app approval.");
    } finally {
      setBusy(false);
    }
  }

  async function linkWallet(wallet: ZcashConnector) {
    if (busy) return;
    setBusy(true);
    try {
      if (!wallet.capabilities.has("identitySigning") || !wallet.signIdentityChallenge || !wallet.connect) throw new Error("Wallet cannot sign this request.");
      setStatus("Approve the Zcash sign-in request in your wallet.");
      await wallet.connect();
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
      await refreshMethods();
      setStatus("Zcash sign-in linked to this Zerant account.");
    } catch {
      setStatus("Wallet approval was not completed.");
    } finally {
      setBusy(false);
    }
  }

  const [handoffUri, setHandoffUri] = useState("");

  function selectWallet(wallet: ZcashConnector) {
    if (wallet.transport === "walletconnect") return;
    if (wallet.capabilities.has("zecAuth")) void linkWalletApp();
    else void linkWallet(wallet);
  }

  return (
    <article className="account-card">
      <p className="eyebrow">Zcash sign-in</p>
      <h2>Linked sign-in methods</h2>
      {methods.length ? (
        <ul>
          {methods.map((method) => (
            <li key={`${method.method}:${method.chain ?? ""}`}>
              {method.method === "zecauth" ? "ZecAuth" : "Wallet message"}
              {method.chain ? ` · ${method.chain.replace("zcash:", "")}` : ""}
              {` · added ${new Date(method.created_at).toLocaleDateString()}`}
              <Button variant="secondary" onClick={() => removeMethod(method)} disabled={busy} aria-label={`Remove ${method.method === "zecauth" ? "ZecAuth" : `${method.chain?.replace("zcash:", "")} wallet message`} sign-in`}>Remove</Button>
            </li>
          ))}
        </ul>
      ) : <p className="muted">No Zcash sign-in method is linked yet.</p>}
      <p className="muted">Linking lets you sign in to this same Zerant account. It does not connect payment information.</p>
      <p className="muted">Removing a method stops it from accessing this Zerant account and signs out all Zcash-authenticated sessions for security. Add a passkey or another sign-in method first if this is your last one. To change keys, remove the old method, then link the new one.</p>
      <ZcashWalletSelector purpose="identity" busy={busy} onSelect={selectWallet} />
      {handoffUri ? <div className="wallet-handoff">
        <textarea readOnly aria-label="ZecAuth link request URI" value={handoffUri} rows={3} />
        <div className="vault-actions wrap">
          <Button variant="secondary" onClick={() => window.location.assign(handoffUri)}>Open in wallet</Button>
          <Button variant="secondary" onClick={() => navigator.clipboard.writeText(handoffUri).then(() => setStatus("Request URI copied."), () => setStatus("Copy failed. Select the URI above."))}>Copy request</Button>
          <Button variant="secondary" onClick={checkWalletAppApproval} disabled={busy}>Check approval</Button>
        </div>
      </div> : null}
      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </article>
  );
}
