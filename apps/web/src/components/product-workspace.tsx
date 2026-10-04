"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { loadVault } from "@/lib/vault-storage";

type VaultState = "checking" | "available" | "empty" | "unavailable";

export function ProductWorkspace() {
  const [vaultState, setVaultState] = useState<VaultState>("checking");

  useEffect(() => {
    let active = true;
    void loadVault()
      .then((vault) => {
        if (!active) return;
        setVaultState(vault ? "available" : "empty");
      })
      .catch(() => {
        if (active) setVaultState("unavailable");
      });
    return () => {
      active = false;
    };
  }, []);

  const vaultCopy =
    vaultState === "checking"
      ? "Checking local encrypted storage…"
      : vaultState === "available"
        ? "Encrypted holder vault detected in this browser."
        : vaultState === "empty"
          ? "No local holder vault exists in this browser."
          : "Local encrypted storage is unavailable in this browser.";

  return (
    <main id="main" className="product-app">
      <section className="app-heading">
        <div>
          <p className="eyebrow">Zerant workspace</p>
          <h1>Trust requests without the data dragnet.</h1>
          <p className="muted">
            This workspace starts empty. Identities, credentials, payments, issuers and verification
            results appear only after the corresponding real integration provides them.
          </p>
        </div>
        <span className="pill">Production surface</span>
      </section>

      <section className="workspace-grid" aria-label="Zerant product workspace">
        <article className="workspace-card workspace-card-primary">
          <div className="workspace-card-top">
            <div>
              <p className="eyebrow">Holder</p>
              <h2>Private vault</h2>
            </div>
            <span className="status-dot" aria-hidden="true" />
          </div>
          <p>{vaultCopy}</p>
          <p className="small muted">
            Credentials and private evidence remain local. Wallet seeds, spending keys, PCZT
            artifacts and FROST shares do not belong in the browser vault.
          </p>
          <Link className="button" href="/vault">Open local vault <span aria-hidden="true">→</span></Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Requests</p>
          <h2>No authenticated request loaded.</h2>
          <p className="muted">
            Zerant will display verifier origin, purpose, requirements and disclosure boundaries
            here after authenticated request transport is connected.
          </p>
          <div className="workspace-state"><span />Waiting for transport</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Issuer</p>
          <h2>No issuer service connected.</h2>
          <p className="muted">
            Issuance, revocation and trust-policy data will appear only from configured issuer
            infrastructure. The web app does not manufacture credentials.
          </p>
          <div className="workspace-state"><span />Not configured</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Verifier</p>
          <h2>No verification session active.</h2>
          <p className="muted">
            Accepted results will come from signed, audience-bound protocol responses. Source
            credentials and unrelated holder data are not fallback inputs.
          </p>
          <div className="workspace-state"><span />No active session</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Zcash</p>
          <h2>No browser wallet authority.</h2>
          <p className="muted">
            Z3/Zallet access belongs behind a trusted native boundary. Browser code does not
            receive RPC credentials, spending keys, wallet history or PCZT bytes.
          </p>
          <div className="workspace-state"><span />Native integration required</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Protocol</p>
          <h2>Native verification core.</h2>
          <p className="muted">
            Credential, policy, disclosure, replay, payment-intent and settlement verification
            remain implemented in Rust. Browser integration will call those boundaries rather
            than duplicating security-critical rules in TypeScript.
          </p>
          <div className="workspace-state ready"><span />Core available</div>
        </article>
      </section>
    </main>
  );
}
