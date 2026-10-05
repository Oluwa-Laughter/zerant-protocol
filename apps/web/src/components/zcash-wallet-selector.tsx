"use client";

import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { discoverZcashConnectors, type WalletPurpose, type ZcashConnector } from "@/lib/zcash-connectors";
import { ensureZcashConfig, getZcashConnectionSnapshot, setZcashDisplayUri } from "@/lib/zcash-connection";

export function ZcashWalletSelector({ purpose, busy = false, hideAuthHandoff = false, triggerLabel = "Connect Zcash wallet", onSelect }: {
  purpose: WalletPurpose;
  busy?: boolean;
  hideAuthHandoff?: boolean;
  triggerLabel?: string;
  onSelect: (connector: ZcashConnector) => void;
}) {
  const [open, setOpen] = useState(false);
  const [connectors, setConnectors] = useState<ZcashConnector[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [activeChain, setActiveChain] = useState<"zcash:mainnet" | "zcash:testnet" | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const first = useRef<HTMLButtonElement>(null);

  function connectorDetail(connector: ZcashConnector): string {
    if (connector.transport === "zecauth") return "Open a compatible wallet app to approve Zerant sign-in";
    if (connector.transport === "uri_handoff") return purpose === "connection"
      ? "Review a Zcash payment request before opening it in your wallet"
      : "Open the complete reviewed Zcash payment request in a compatible wallet";
    if (connector.transport === "walletconnect") return "Pair a compatible remote wallet";
    if (connector.capabilities.has("identitySigning")) return "Installed wallet · supports Zerant sign-in";
    if (connector.capabilities.has("shieldedPayment")) return "Installed wallet · supports shielded payments";
    return "Installed wallet · available for supported Zcash actions";
  }

  useEffect(() => {
    if (open) first.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open || !activeChain) return;

    const refresh = () => {
      const { walletConnectProjectId } = getZcashConnectionSnapshot();
      setConnectors(
        discoverZcashConnectors(
          purpose,
          activeChain,
          walletConnectProjectId,
          undefined,
          setZcashDisplayUri,
        ).filter((connector) => !hideAuthHandoff || !connector.capabilities.has("zecAuth")),
      );
    };

    // Browser extensions may inject after React has already rendered. Noir emits this
    // event once its provider is ready, so refresh instead of forcing users to reload.
    window.addEventListener("noirwallet#initialized", refresh);
    const retry = window.setTimeout(refresh, 750);
    return () => {
      window.removeEventListener("noirwallet#initialized", refresh);
      window.clearTimeout(retry);
    };
  }, [activeChain, hideAuthHandoff, open, purpose]);

  function close() {
    setOpen(false);
    requestAnimationFrame(() => trigger.current?.focus());
  }

  return (
    <div className="zcash-wallet-selector">
      <Button ref={trigger} onClick={() => {
        setLoading(true); setError("");
        void ensureZcashConfig().then((chain) => {
          setActiveChain(chain);
          setConnectors(discoverZcashConnectors(purpose, chain, getZcashConnectionSnapshot().walletConnectProjectId, undefined, setZcashDisplayUri)
            .filter((connector) => !hideAuthHandoff || !connector.capabilities.has("zecAuth")));
          setOpen(true);
        }).catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Wallet configuration is unavailable."))
          .finally(() => setLoading(false));
      }} disabled={busy || loading} aria-expanded={open} aria-controls={`zcash-wallet-options-${purpose}`}>
        {loading ? "Finding wallets…" : triggerLabel}
      </Button>
      {error ? <p role="status" className="vault-status neutral">{error}</p> : null}
      {open ? (
        <div id={`zcash-wallet-options-${purpose}`} className="wallet-selector-panel"
          role="group" aria-label={purpose === "identity" ? "Choose a Zcash sign-in wallet" : purpose === "payment" ? "Choose a Zcash payment wallet" : "Choose a Zcash connection"}
          onKeyDown={(event) => { if (event.key === "Escape") { event.preventDefault(); close(); } }}>
          <div className="wallet-selector-heading">
            <div>
              <h3>{purpose === "identity" ? "Choose a sign-in wallet" : purpose === "payment" ? "Choose a payment wallet" : "Choose a Zcash wallet"}</h3>
              <p className="small muted">{purpose === "identity" ? "Only supported sign-in methods are listed." : "Select a wallet or a supported handoff."}</p>
            </div>
            <button type="button" className="wallet-selector-close" onClick={close} aria-label="Close wallet choices">Close</button>
          </div>
          <div className="wallet-selector-list">
            {connectors.length === 0 && purpose === "identity" ? (
              <div className="wallet-selector-empty">
                <strong>No supported Zcash sign-in wallet detected.</strong>
                <p className="small muted">On desktop, unlock Noir Wallet and reload this page. On phones, use a passkey for Zerant access until a supported mobile Zcash sign-in wallet is available.</p>
              </div>
            ) : null}
            {connectors.map((connector, index) => (
              <button key={connector.id} ref={index === 0 ? first : undefined} type="button" className="wallet-selector-choice"
                disabled={busy} onClick={() => { close(); onSelect(connector); }}>
                <span className="wallet-selector-choice-copy">
                  <span className="wallet-selector-choice-title">{connector.name}</span>
                  <span className="wallet-selector-choice-detail">{connectorDetail(connector)}</span>
                </span>
                {connector.availability === "detected" ? <span className="wallet-selector-detected">Detected</span> : null}
                <span className="wallet-selector-arrow" aria-hidden="true">→</span>
              </button>
            ))}
          </div>
          {purpose === "identity" ? (
            <p className="small muted">Payment-only wallets can open payment requests after you sign in. Portable sign-in requires a wallet that supports Zerant wallet approval.</p>
          ) : purpose === "payment" ? (
            <p className="small muted">The portable option opens the complete validated Zcash payment request in a compatible wallet. Wallet submission is not settlement confirmation.</p>
          ) : activeChain === "zcash:testnet" ? (
            <p className="small muted">Testnet uses installed-wallet actions and portable payment handoff. Remote wallet pairing is intentionally reserved for supported mainnet wallets.</p>
          ) : (
            <p className="small muted">Choose an installed wallet, a portable payment handoff, or remote wallet pairing when available.</p>
          )}
        </div>
      ) : null}
    </div>
  );
}
