"use client";

import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { ZcashWalletSelector } from "@/components/zcash-wallet-selector";
import { signInWithZcashWallet, type SignInChallenge, type ZcashSignInStage } from "@/lib/zcash-auth";
import { discoverZcashConnectors, type ZcashConnector } from "@/lib/zcash-connectors";
import { classifyInjectedWalletError } from "@/lib/zcash-wallet";
import { connectConnector, disconnectZcash, ensureZcashConfig, getZcashConnectionSnapshot, resetConnectorAuthorization, useZcashConnection } from "@/lib/zcash-connection";

function signInStageCopy(stage: ZcashSignInStage): string {
  switch (stage) {
    case "challenge": return "Creating a short-lived Zerant sign-in request…";
    case "wallet_approval": return "Waiting for Noir to approve the private identity signature…";
    case "verification": return "Noir approved. Verifying the signature against this Zerant request…";
    case "session": return "Signature verified. Completing your Zerant session…";
  }
}

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
    // Load only Zerant's public network configuration here. Do not touch the
    // injected wallet before the user's explicit connection gesture.
    void ensureZcashConfig().catch(() => undefined);
  }, []);

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
    setStatus(connector.walletId === "noir"
      ? "Check Noir’s Connect Request. Select a Testnet account and click Connect in the wallet."
      : "Opening wallet connection…");
    let chain: "zcash:mainnet" | "zcash:testnet" | null = null;
    try {
      chain = getZcashConnectionSnapshot().activeChain;
      if (!chain) throw new Error("Zcash network configuration is still loading. Reopen the wallet menu and try again.");
      await connectConnector(connector, chain);
      setStatus(connector.capabilities.has("identitySigning")
        ? "Wallet connected. Continue with the separate Zerant sign-in approval."
        : "Wallet available for supported Zcash actions. Your Zerant account remains independent of this wallet session.");
    } catch (error) {
      const message = error instanceof Error ? error.message : "";
      const kind = classifyInjectedWalletError(error);
      if (connector.walletId === "noir" && kind === "rejected") {
        setStatus("Noir’s Connect Request was closed or rejected. Select a Testnet account and click Connect in Noir to try again.");
      } else if (connector.walletId === "noir" && kind === "unauthorized") {
        setStatus(chain === "zcash:testnet"
          ? "Testnet Noir has not authorized this site yet. Unlock the Testnet Noir extension, choose Connect again, and approve Zerant when the wallet prompt opens."
          : "Noir has not authorized this site yet. Unlock Noir, choose Connect again, and approve Zerant when the wallet prompt opens.");
      } else if (connector.walletId === "noir" && kind === "pending") {
        setStatus("Noir already has a connection request waiting. Open the extension and finish or dismiss that request before trying again.");
      } else if (connector.walletId === "noir" && /no wallets available to authori[sz]e/i.test(message)) {
        setStatus(chain === "zcash:testnet"
          ? "Testnet Noir has no account ready to share. Open the Testnet Noir extension, create or unlock a Testnet account, then choose Connect again."
          : "Noir has no account ready to share. Open the extension, create or unlock an account, then choose Connect again.");
      } else {
        setStatus(message || "Wallet connection failed.");
      }
    }
    finally { setBusy(false); }
  }

  async function resetNoirAuthorization() {
    if (busy) return;
    setBusy(true);
    setHandoffUri("");
    setStatus("Resetting Testnet Noir site authorization…");
    try {
      const chain = await ensureZcashConfig();
      const { walletConnectProjectId } = getZcashConnectionSnapshot();
      const noir = connection.selected?.walletId === "noir"
        ? connection.selected
        : discoverZcashConnectors(purpose, chain, walletConnectProjectId)
            .find((connector) => connector.walletId === "noir" && typeof connector.disconnect === "function");

      if (!noir?.disconnect) {
        setStatus("Testnet Noir is not detected in this browser. Unlock the extension, reload Zerant, then try again.");
        return;
      }

      const resetState = await resetConnectorAuthorization(noir, chain);
      if (resetState === "not_authorized") {
        setStatus("Noir site authorization was cleared and verified. Zerant did not change your wallet funds or history. Choose Testnet Noir again and approve a fresh Connect Request.");
      } else if (resetState === "ready" || resetState === "wrong_network") {
        setStatus("Noir still reports this site as authorized after reset. Open Noir’s connected-dApp controls, remove Zerant there, reload this page, then reconnect.");
      } else {
        setStatus("Noir accepted the reset, but Zerant could not confirm that the site permission was cleared. Reload this page and reconnect; if it still fails, remove Zerant from Noir’s connected-dApp controls.");
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : "";
      setStatus(message || "Noir authorization could not be reset. Unlock Testnet Noir and try again.");
    } finally {
      setBusy(false);
    }
  }

  async function signIn() {
    if (!connection.selected?.capabilities.has("identitySigning") || busy) return;
    setBusy(true);
    setStatus("Approve Zerant sign-in in your wallet…");
    try {
      await signInWithZcashWallet(connection.selected, fetch, (stage) => setStatus(signInStageCopy(stage)));
      setStatus("Signed in to Zerant.");
      onConnected?.();
    } catch (error) {
      const message = error instanceof Error ? error.message : "";
      const kind = classifyInjectedWalletError(error);
      if (kind === "rejected") {
        setStatus("Noir closed or rejected the sign-in request. Reopen Testnet Noir and approve the identity-signature prompt when you are ready.");
      } else if (kind === "unauthorized") {
        setStatus("Noir is detected but this site is no longer authorized. Disconnect the wallet here, reconnect Testnet Noir, then start sign-in again.");
      } else if (kind === "pending") {
        setStatus("Noir already has a sign-in request waiting. Open the extension and finish or dismiss that request before trying again.");
      } else {
        setStatus(message || "Wallet sign-in failed.");
      }
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
      <p className="muted">{allowSignIn ? "On desktop, Zerant signs in through a detected Noir Wallet using a privacy-preserving identity key. A payment address, balance, or history is never your Zerant identity." : "Choose Noir to approve this site for wallet actions. You can also hand a reviewed payment request to a compatible wallet app. Your Zerant account and credentials work without a wallet connection."}</p>
    </div>
    <ZcashWalletSelector purpose={purpose} busy={busy} hideAuthHandoff={!allowSignIn} hidePaymentHandoff={!allowSignIn} triggerLabel={allowSignIn ? "Choose sign-in wallet" : "Connect a direct wallet"} onSelect={(connector) => void choose(connector)} />
    {connection.activeChain === "zcash:testnet" && !connection.account ? <p className="small muted">Direct connection requires the separate [Testnet] Noir Wallet extension. <a href="https://docs.zknoir.com/developers/" target="_blank" rel="noopener noreferrer">Get the official testnet setup guide ↗</a></p> : null}
    {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    {connection.account && (!allowSignIn || connection.selected?.capabilities.has("identitySigning")) ? <div className="zcash-connection-state"><p className="small">Connected to <strong>{connection.account.providerName}</strong>{connection.selected?.capabilities.has("shieldedPayment") ? ". Shielded payments available." : "."}</p><div className="vault-actions wrap">{allowSignIn ? <Button onClick={() => void signIn()} disabled={busy}>Sign in with wallet</Button> : null}<Button variant="secondary" onClick={() => void disconnectZcash()} disabled={busy}>Disconnect wallet</Button></div></div> : null}
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
    {connection.activeChain === "zcash:testnet" && !connection.account ? (
      <details className="noir-connect-checklist">
        <summary>How to approve in Testnet Noir</summary>
        <ol>
          <li>Unlock <strong>[Testnet] Noir Wallet</strong>.</li>
          <li>Keep Noir’s Connect Request popup open.</li>
          <li>If no account is selected, choose <strong>Edit accounts</strong> and select at least one Testnet account.</li>
          <li>Click <strong>Connect</strong> inside Noir. Closing the popup first rejects the request.</li>
        </ol>
        <p className="small muted">Noir’s permission screen may mention balances and activity. Zerant does not request or store them for identity.</p>
      </details>
    ) : null}
    {!connection.account ? <details className="wallet-recovery-option">
      <summary>Having trouble connecting?</summary>
      <p className="small muted">If Noir keeps ending the Connect Request, clear this site’s Noir permission and reconnect. This does not delete your Zerant account or touch wallet funds.</p>
      <Button variant="secondary" disabled={busy} onClick={() => void resetNoirAuthorization()}>Reset Noir permission</Button>
    </details> : null}
    <details className="wallet-connection-details">
      <summary>Connection details</summary>
      <div className="zcash-connection-diagnostics" aria-label="Zcash wallet connection diagnostics">
        <div>
          <span className="eyebrow">Zerant network</span>
          <strong>{connection.activeChain === "zcash:testnet" ? "Testnet" : connection.activeChain === "zcash:mainnet" ? "Mainnet" : "Loading…"}</strong>
        </div>
        <div>
          <span className="eyebrow">Wallet authorization</span>
          <strong>{connection.status === "connected" ? "Authorized" : connection.status === "connecting" ? "Approval in progress" : "Not connected"}</strong>
        </div>
        <div>
          <span className="eyebrow">Zerant sign-in</span>
          <strong>{allowSignIn && connection.account && connection.selected?.capabilities.has("identitySigning") ? "Ready" : allowSignIn ? "Wallet connection required" : "Not required"}</strong>
        </div>
      </div>
    </details>
  </div>;
}
