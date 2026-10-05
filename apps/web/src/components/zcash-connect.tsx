"use client";

import { useEffect, useRef, useState } from "react";
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

export function ZcashConnect({ purpose, onConnected }: { purpose: "identity" | "connection"; onConnected?: () => void }) {
  const connection = useZcashConnection();
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const [handoffUri, setHandoffUri] = useState("");
  const openHandoff = useRef<HTMLButtonElement>(null);
  const allowSignIn = purpose === "identity";

  useEffect(() => { if (handoffUri) openHandoff.current?.focus(); }, [handoffUri]);

  useEffect(() => {
    if (purpose !== "connection") return;
    let active = true;
    void ensureZcashConfig().then((chain) => {
      if (!active) return;
      const { walletConnectProjectId } = getZcashConnectionSnapshot();
      return restoreConnection(discoverZcashConnectors("connection", chain, walletConnectProjectId));
    }).catch(() => { /* The selector reports configuration errors when opened. */ });
    return () => { active = false; };
  }, [purpose]);

  async function choose(connector: ZcashConnector) {
    if (busy) return;
    setHandoffUri("");
    if (connector.capabilities.has("zip321Handoff")) {
      setStatus("Paste and validate a ZIP-321 request in the payment review below before opening it in your wallet.");
      requestAnimationFrame(() => document.getElementById("zcash-payment-uri")?.focus());
      return;
    }
    if (connector.capabilities.has("zecAuth")) {
      if (!allowSignIn) { setStatus("Wallet-app sign-in is available from account settings."); return; }
      setBusy(true);
      setStatus("Preparing wallet-app sign-in request…");
      try {
        const challenge = await createChallenge();
        setHandoffUri(connector.openAuthHandoff!(challenge, window.location.origin + "/api/zerant/auth/verify"));
        setStatus("Open this request in the supported wallet, approve sign-in there, then check approval here.");
      } catch (error) { setStatus(error instanceof Error ? error.message : "Could not open wallet app."); }
      finally { setBusy(false); }
      return;
    }
    setBusy(true);
    setStatus("Opening wallet connection…");
    let chain: "zcash:mainnet" | "zcash:testnet" | null = null;
    try {
      chain = await ensureZcashConfig();
      await connectConnector(connector);
      setStatus(connector.capabilities.has("identitySigning")
        ? "Wallet connected. Continue with the separate Zerant sign-in approval."
        : "Wallet available for supported Zcash actions. Your Zerant account remains independent of this wallet session.");
    } catch (error) {
      const message = error instanceof Error ? error.message : "";
      if (/rejected/i.test(message) && connector.walletId === "noir") {
        setStatus(chain === "zcash:testnet"
          ? "Noir did not authorize this connection. Zerant is on Zcash testnet, so make sure you are using the separate Testnet Noir Wallet build, unlock it, then approve this site when Noir opens."
          : "Noir did not authorize this connection. Unlock Noir Wallet and approve this site when the account-access request opens.");
      } else {
        setStatus(message || "Wallet connection failed.");
      }
    }
    finally { setBusy(false); }
  }

  async function signIn() {
    if (!connection.selected?.capabilities.has("identitySigning") || busy) return;
    setBusy(true);
    setStatus("Approve Zerant sign-in in your wallet…");
    try {
      await signInWithZcashWallet(connection.selected);
      setStatus("Signed in to Zerant.");
      onConnected?.();
    } catch (error) {
      const message = error instanceof Error ? error.message : "";
      setStatus(/rejected/i.test(message)
        ? "The wallet did not approve the sign-in request. If you did not reject it, unlock Noir Wallet, confirm this site is connected, and try again."
        : message || "Wallet sign-in failed.");
    }
    finally { setBusy(false); }
  }

  async function checkWalletApproval() {
    if (busy) return;
    setBusy(true);
    setStatus("Checking wallet-app approval…");
    try {
      const response = await fetch("/api/zerant/auth/session", { credentials: "same-origin", cache: "no-store" });
      if (!response.ok) {
        setStatus("No wallet-app approval is ready yet. Approve sign-in in your wallet, then check again.");
        return;
      }
      setStatus("Signed in to Zerant."); onConnected?.();
    } catch { setStatus("Could not check wallet-app approval. Try again."); }
    finally { setBusy(false); }
  }

  return <div className="zcash-connect">
    <div className="zcash-connect-intro">
      <h2>{allowSignIn ? "Sign in with a Zcash wallet" : "Use a Zcash wallet"}</h2>
      <p className="muted">{allowSignIn ? "On desktop, Zerant signs in through a detected Noir Wallet using a privacy-preserving identity key. A payment address, balance, or history is never your Zerant identity." : "Wallet access is optional. Use an installed wallet when available, or hand a reviewed payment request to a compatible Zcash wallet. Your credentials and Zerant ID do not depend on a wallet session."}</p>
    </div>
    <ZcashWalletSelector purpose={purpose} busy={busy} hideAuthHandoff={!allowSignIn} triggerLabel={allowSignIn ? "Choose sign-in wallet" : "Choose wallet option"} onSelect={(connector) => void choose(connector)} />
    {handoffUri ? <div className="wallet-handoff zcash-connect-handoff">
      <p className="small muted">This sign-in request expires after five minutes. It contains the challenge and callback, not your wallet address or history.</p>
      <textarea readOnly aria-label="ZecAuth sign-in request URI" value={handoffUri} rows={3} />
      <div className="vault-actions wrap">
        <Button ref={openHandoff} onClick={() => window.location.assign(handoffUri)}>Open in wallet app</Button>
        <Button variant="secondary" onClick={() => void navigator.clipboard.writeText(handoffUri).then(() => setStatus("Sign-in request copied."), () => setStatus("Copy failed. Select the request URI above."))}>Copy sign-in request</Button>
        <Button variant="secondary" onClick={() => void checkWalletApproval()} disabled={busy}>Check approval</Button>
      </div>
    </div> : null}
    {!allowSignIn && connection.displayUri ? <div className="wallet-connect-option"><h3>Pair in your wallet</h3><p className="small muted">Only WalletConnect-compatible Zcash wallets can approve this request.</p><textarea readOnly aria-label="WalletConnect pairing URI" value={connection.displayUri} rows={3} /><Button variant="secondary" onClick={() => void navigator.clipboard.writeText(connection.displayUri!).then(() => setStatus("Connection link copied."), () => setStatus("Copy failed. Select the connection link above."))}>Copy connection link</Button></div> : null}
    {connection.account && (!allowSignIn || connection.selected?.capabilities.has("identitySigning")) ? <div className="zcash-connection-state"><p className="small">Connected to <strong>{connection.account.providerName}</strong>{connection.selected?.capabilities.has("shieldedPayment") ? ". Shielded payments available." : "."}</p><div className="vault-actions wrap">{allowSignIn ? <Button onClick={() => void signIn()} disabled={busy}>Sign in with wallet</Button> : null}<Button variant="secondary" onClick={() => void disconnectZcash()} disabled={busy}>Disconnect wallet</Button></div></div> : null}
    {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
  </div>;
}
