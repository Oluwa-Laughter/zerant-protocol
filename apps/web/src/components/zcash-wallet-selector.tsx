"use client";

import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { discoverZcashConnectors, type WalletPurpose, type ZcashConnector } from "@/lib/zcash-connectors";
import { ensureZcashConfig, getZcashConnectionSnapshot, setZcashDisplayUri } from "@/lib/zcash-connection";

export function ZcashWalletSelector({ purpose, busy = false, hideAuthHandoff = false, hidePaymentHandoff = false, triggerLabel = "Connect Zcash wallet", onSelect }: {
  purpose: WalletPurpose;
  busy?: boolean;
  hideAuthHandoff?: boolean;
  hidePaymentHandoff?: boolean;
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
      ? "Review a payment request first; this option does not connect a wallet"
      : "Open the complete reviewed Zcash payment request in a compatible wallet";
    if (connector.transport === "walletconnect") return "Pair a compatible remote wallet";
    if (connector.capabilities.has("identitySigning")) return activeChain === "zcash:testnet"
      ? "Installed wallet · Zerant testnet requires the Testnet Noir Wallet build"
      : "Installed wallet · supports Zerant sign-in";
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
      const discovered = discoverZcashConnectors(
        purpose,
        activeChain,
        walletConnectProjectId,
        undefined,
        setZcashDisplayUri,
      ).filter((connector) =>
        (!hideAuthHandoff || !connector.capabilities.has("zecAuth")) &&
        (!hidePaymentHandoff || !connector.capabilities.has("zip321Handoff")));
      setConnectors(discovered);
    };

    // Browser extensions may inject after React has already rendered. Noir emits this
    // event once its provider is ready, so refresh instead of forcing users to reload.
    window.addEventListener("noirwallet#initialized", refresh);
    const retry = window.setTimeout(refresh, 750);
    return () => {
      window.removeEventListener("noirwallet#initialized", refresh);
      window.clearTimeout(retry);
    };
  }, [activeChain, hideAuthHandoff, hidePaymentHandoff, open, purpose]);

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
            .filter((connector) =>
              (!hideAuthHandoff || !connector.capabilities.has("zecAuth")) &&
              (!hidePaymentHandoff || !connector.capabilities.has("zip321Handoff"))));
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
              <p className="small muted">{purpose === "identity" ? "Only supported sign-in methods are listed." : hidePaymentHandoff ? "Only direct wallet connections are listed." : "Select a wallet or a supported payment handoff."}</p>
            </div>
            <button type="button" className="wallet-selector-close" onClick={close} aria-label="Close wallet choices">Close</button>
          </div>
          <div className="wallet-selector-list">
            {connectors.length === 0 && purpose === "identity" ? (
              <div className="wallet-selector-empty">
                <strong>No supported Zcash sign-in wallet detected.</strong>
                <p className="small muted">On desktop Chrome, enable and unlock the Testnet Noir Wallet extension, allow it on this site, then reopen this menu. You can still use your passkey to access Zerant.</p>
              </div>
            ) : null}
            {connectors.length === 0 && purpose === "connection" ? (
              <div className="wallet-selector-empty">
                <strong>No direct wallet connection found.</strong>
                <p className="small muted">You can still prepare a payment and copy its reviewed link for a compatible Zcash wallet. Your Zerant account and credentials remain available.</p>
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
            <p className="small muted">{activeChain === "zcash:testnet"
              ? "Zerant is currently using Zcash testnet. Noir Wallet uses separate mainnet and testnet extensions, so sign-in requires the Testnet Noir Wallet build."
              : "Only wallets that support Zerant's privacy-preserving identity signature are offered for sign-in."}</p>
          ) : purpose === "payment" ? (
            <p className="small muted">The portable option opens the complete validated Zcash payment request in a compatible wallet. Wallet submission is not settlement confirmation.</p>
          ) : activeChain === "zcash:testnet" ? (
            <p className="small muted">The Chrome Store Noir extension is mainnet and cannot pay testnet requests. Direct actions require the separate Testnet Noir build. A payment link works with a compatible testnet wallet without a live connection.</p>
          ) : (
            <p className="small muted">Choose an installed wallet or remote pairing when available. Review a payment request above to use a portable wallet link.</p>
          )}
        </div>
      ) : null}
    </div>
  );
}
