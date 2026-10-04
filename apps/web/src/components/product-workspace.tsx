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
        if (active) setVaultState(vault ? "available" : "empty");
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
      ? "Checking encrypted storage in this browser…"
      : vaultState === "available"
        ? "Your encrypted local holder vault is available on this device."
        : vaultState === "empty"
          ? "Create a local encrypted vault before receiving private credentials."
          : "This browser cannot access Zerant local encrypted storage.";

  return (
    <main id="main" className="product-app">
      <section className="app-heading workspace-intro">
        <div>
          <p className="eyebrow">Zerant workspace</p>
          <h1>Your private trust workspace.</h1>
          <p>
            Zerant brings together the holder&apos;s private evidence, incoming verification
            requests, issuer relationships and Zcash settlement state without combining them into
            one public identity profile.
          </p>
        </div>
        <span className="pill">Real state only</span>
      </section>

      <section className="workspace-explainer">
        <div>
          <span className="eyebrow">What happens here</span>
          <h2>Receive. Review. Prove.</h2>
        </div>
        <ol>
          <li><strong>Receive trusted credentials</strong><span>Credentials come from connected issuers and stay under holder control.</span></li>
          <li><strong>Review incoming requests</strong><span>See who is asking, the purpose, and exactly what result would be disclosed.</span></li>
          <li><strong>Approve only what is needed</strong><span>Zerant returns the bounded result after native verification checks pass.</span></li>
        </ol>
      </section>

      <section className="workspace-grid" aria-label="Zerant product workspace">
        <article className="workspace-card workspace-card-primary">
          <div className="workspace-card-top">
            <div><p className="eyebrow">Holder vault</p><h2>Your private evidence</h2></div>
            <span className="status-dot" aria-hidden="true" />
          </div>
          <p>{vaultCopy}</p>
          <p className="small muted">
            The browser vault stores credentials and evidence only. Zcash seeds, spending keys,
            PCZT artifacts and FROST shares stay outside it.
          </p>
          <Link className="button" href="/vault">{vaultState === "available" ? "Open vault" : "Set up vault"} <span aria-hidden="true">→</span></Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Verification requests</p>
          <h2>No request waiting for you.</h2>
          <p className="muted">
            When authenticated request transport is connected, incoming requests will appear here
            with requester identity, purpose, requested conditions and disclosure preview.
          </p>
          <div className="workspace-state"><span />Waiting for a real request</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Trusted issuers</p>
          <h2>No issuer connected.</h2>
          <p className="muted">
            Connected issuers will be able to issue and revoke credentials they are authorized to
            make. Zerant will not manufacture credentials in the browser.
          </p>
          <div className="workspace-state"><span />Issuer connection required</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Verification history</p>
          <h2>No verification session active.</h2>
          <p className="muted">
            Accepted results are audience-bound and replay-protected. Source credentials and
            unrelated holder data are never treated as fallback verifier inputs.
          </p>
          <div className="workspace-state"><span />No active verification</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Zcash settlement</p>
          <h2>No native wallet connection.</h2>
          <p className="muted">
            Z3/Zallet settlement lives behind the trusted native boundary so browser code does not
            receive RPC credentials, spending keys, wallet history or PCZT bytes.
          </p>
          <div className="workspace-state"><span />Native connection required</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Protocol security</p>
          <h2>Native verification is ready.</h2>
          <p className="muted">
            Credential, policy, disclosure, replay, payment-intent and settlement verification are
            implemented in Rust and remain the source of truth for security-critical decisions.
          </p>
          <div className="workspace-state ready"><span />Core verification available</div>
        </article>
      </section>
    </main>
  );
}
