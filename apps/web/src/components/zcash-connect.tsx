"use client";

import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { ZcashWalletSelector } from "@/components/zcash-wallet-selector";
import { signInWithZcashWallet, type SignInChallenge } from "@/lib/zcash-auth";
import { discoverZcashConnectors, type ZcashConnector } from "@/lib/zcash-connectors";
import { connectConnector, disconnectZcash, ensureZcashConfig, getZcashConnectionSnapshot, restoreConnection, useZcashConnection } from "@/lib/zcash-connection";

async function createChallenge(): Promise<SignInChallenge> {
  const response = await fetch("/api/zerant/auth/challenge?scopes=signin", { credentials: "same-origin", cache: "no-store" });
  if (!response.ok) throw new Error("Zerant could not create a wallet sign-in request.");
  return response.json() as Promise<SignInChallenge>;
}

export function ZcashConnect({ onConnected, allowSignIn = true }: { onConnected?: () => void; allowSignIn?: boolean }) {
  const connection = useZcashConnection();
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let active = true;
    void ensureZcashConfig().then((chain) => {
      if (!active) return;
      const { walletConnectProjectId } = getZcashConnectionSnapshot();
      return restoreConnection(discoverZcashConnectors("connection", chain, walletConnectProjectId));
    }).catch(() => { /* The selector reports configuration errors when opened. */ });
    return () => { active = false; };
  }, []);

  async function choose(connector: ZcashConnector) {
    if (busy) return;
    if (connector.capabilities.has("zip321Handoff")) {
      setStatus("Use the payment request review below to open a complete ZIP-321 request in your wallet.");
      return;
    }
    if (connector.capabilities.has("zecAuth")) {
      if (!allowSignIn) { setStatus("Wallet-app sign-in is available from account settings."); return; }
      setBusy(true);
      try {
        const challenge = await createChallenge();
        window.location.assign(connector.openAuthHandoff!(challenge, window.location.origin + "/api/zerant/auth/verify"));
      } catch (error) { setStatus(error instanceof Error ? error.message : "Could not open wallet app."); }
      finally { setBusy(false); }
      return;
    }
    setBusy(true);
    try {
      await connectConnector(connector);
      setStatus(connector.capabilities.has("identitySigning")
        ? "Wallet connected. You can approve a separate Zerant sign-in."
        : "Wallet connected for its supported Zcash actions. Use a passkey for Zerant account access.");
    } catch (error) { setStatus(error instanceof Error ? error.message : "Wallet connection failed."); }
    finally { setBusy(false); }
  }

  async function signIn() {
    if (!connection.selected?.capabilities.has("identitySigning") || busy) return;
    setBusy(true);
    try {
      await signInWithZcashWallet(connection.selected);
      setStatus("Signed in to Zerant.");
      onConnected?.();
    } catch (error) { setStatus(error instanceof Error ? error.message : "Wallet sign-in failed."); }
    finally { setBusy(false); }
  }

  async function checkWalletApproval() {
    try {
      const response = await fetch("/api/zerant/auth/session", { credentials: "same-origin", cache: "no-store" });
      if (!response.ok) throw new Error("No completed wallet approval is ready yet.");
      setStatus("Signed in to Zerant."); onConnected?.();
    } catch (error) { setStatus(error instanceof Error ? error.message : "No approval yet."); }
  }

  return <div className="zcash-connect">
    <p className="eyebrow">Zcash wallet</p>
    <h2>Connect Zcash wallet</h2>
    <p className="muted">Connect for supported Zcash actions. A payment address, balance, or history is not your Zerant identity.</p>
    <ZcashWalletSelector purpose="connection" busy={busy} onSelect={(connector) => void choose(connector)} />
    {connection.displayUri ? <div className="wallet-connect-option"><h3>Scan or copy in your wallet</h3><p className="small muted">Only WalletConnect-compatible Zcash wallets can approve this request.</p><textarea readOnly aria-label="WalletConnect pairing URI" value={connection.displayUri} rows={3} /><Button variant="secondary" onClick={() => void navigator.clipboard.writeText(connection.displayUri!)}>Copy connection link</Button></div> : null}
    {connection.account ? <div className="vault-actions wrap"><p className="small">Connected: {connection.account.providerName}. {connection.selected?.capabilities.has("shieldedPayment") ? "Shielded payments available." : ""}</p>{allowSignIn && connection.selected?.capabilities.has("identitySigning") ? <Button onClick={() => void signIn()} disabled={busy}>Sign in with wallet</Button> : null}<Button variant="secondary" onClick={() => void disconnectZcash()} disabled={busy}>Disconnect</Button></div> : null}
    {allowSignIn ? <Button variant="secondary" onClick={() => void checkWalletApproval()}>Check wallet-app approval</Button> : null}
    {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
  </div>;
}
