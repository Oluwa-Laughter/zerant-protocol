"use client";

import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { discoverZcashConnectors, type WalletPurpose, type ZcashConnector } from "@/lib/zcash-connectors";
import { ensureZcashConfig, getZcashConnectionSnapshot, setZcashDisplayUri } from "@/lib/zcash-connection";

export function ZcashWalletSelector({ purpose, busy = false, triggerLabel = "Connect Zcash wallet", onSelect }: {
  purpose: WalletPurpose;
  busy?: boolean;
  triggerLabel?: string;
  onSelect: (connector: ZcashConnector) => void;
}) {
  const [open, setOpen] = useState(false);
  const [connectors, setConnectors] = useState<ZcashConnector[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const first = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (open) first.current?.focus();
  }, [open]);

  function close() {
    setOpen(false);
    requestAnimationFrame(() => trigger.current?.focus());
  }

  return (
    <div className="zcash-wallet-selector">
      <Button ref={trigger} onClick={() => {
        setLoading(true); setError("");
        void ensureZcashConfig().then((chain) => {
          setConnectors(discoverZcashConnectors(purpose, chain, getZcashConnectionSnapshot().walletConnectProjectId, undefined, setZcashDisplayUri));
          setOpen(true);
        }).catch((reason: unknown) => setError(reason instanceof Error ? reason.message : "Wallet configuration is unavailable."))
          .finally(() => setLoading(false));
      }} disabled={busy || loading} aria-expanded={open} aria-controls={`zcash-wallet-options-${purpose}`}>
        {triggerLabel}
      </Button>
      {error ? <p role="status" className="vault-status neutral">{error}</p> : null}
      {open ? (
        <div id={`zcash-wallet-options-${purpose}`} className="wallet-selector-panel"
          role="group" aria-label={purpose === "identity" ? "Choose a Zcash sign-in wallet" : purpose === "payment" ? "Choose a Zcash payment wallet" : "Choose a Zcash connection"}
          onKeyDown={(event) => { if (event.key === "Escape") { event.preventDefault(); close(); } }}>
          <div className="vault-actions wrap">
            <strong>Choose a Zcash wallet</strong>
            <Button variant="secondary" onClick={close} aria-label="Close wallet choices">Close</Button>
          </div>
          {connectors.map((connector, index) => (
            <Button key={connector.id} ref={index === 0 ? first : undefined} variant="secondary"
              disabled={busy} onClick={() => { close(); onSelect(connector); }}>
              {connector.name}{connector.availability === "detected" ? " · detected" : ""}
            </Button>
          ))}
          {purpose === "identity" ? (
            <p className="small muted">Payment-only wallets can still open ZIP-321 requests after you sign in with a passkey. Portable sign-in requires a wallet that implements ZecAuth.</p>
          ) : purpose === "payment" ? (
            <p className="small muted">The portable option opens the complete validated ZIP-321 request in a compatible wallet. Wallet submission is not settlement confirmation.</p>
          ) : <p className="small muted">Choose an installed wallet, a supported remote wallet, or a portable Zcash action.</p>}
        </div>
      ) : null}
    </div>
  );
}
