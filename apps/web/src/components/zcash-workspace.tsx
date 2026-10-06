"use client";

import { useState } from "react";
import Link from "next/link";
import { ZcashAddressInspector } from "@/components/zcash-address-inspector";
import { ZcashConnect } from "@/components/zcash-connect";
import { ZcashPaymentRequestReview } from "@/components/zcash-payment-request-review";
import { ZcashInvoiceManager } from "@/components/zcash-invoice-manager";
import { ZcashLiveStatus } from "@/components/zcash-live-status";
import { WorkspaceSectionNav } from "@/components/workspace-section-nav";
import type { WorkspaceSession, WorkspaceZcash, WorkspaceZcashNetwork } from "@/components/product-workspace";

function readinessCopy(network: WorkspaceZcashNetwork, zcash: WorkspaceZcash) {
  if (network?.state === "ready") return { label: "Ready", title: "Zcash network ready.", body: "Zerant has a fresh network view for supported Zcash actions." };
  if (network?.state === "syncing") return { label: "Syncing", title: "Zcash network syncing.", body: "Network-dependent actions stay protected while Zerant catches up." };
  if (network?.state === "degraded") return { label: "Protected mode", title: "Zcash network temporarily unavailable.", body: "Zerant keeps network-dependent actions disabled until a fresh check succeeds." };
  if (zcash) return { label: "Available", title: "Zcash services available.", body: "Zerant can perform supported Zcash validation and wallet actions." };
  return { label: "Unavailable", title: "Zcash network connection unavailable.", body: "Private credentials still work, but Zcash network actions are unavailable." };
}

export function ZcashWorkspace({ session, zcash, network, activeSection }: { session: WorkspaceSession; zcash: WorkspaceZcash; network: WorkspaceZcashNetwork; activeSection?: "overview" | "payments" | "invoices" | "wallet" | "address" }) {
  const authenticated = Boolean(session?.authenticated);
  const [liveNetwork, setLiveNetwork] = useState<WorkspaceZcashNetwork>(network);
  const readiness = readinessCopy(liveNetwork, zcash);
  const networkActionsReady = liveNetwork
    ? liveNetwork.network_actions_enabled
    : Boolean(zcash);
  const showAllSections = activeSection === undefined;
  const showOverview = showAllSections || activeSection === "overview";
  const showSection = (section: "payments" | "invoices" | "wallet" | "address") => activeSection === section || (showAllSections && section !== "wallet");

  return (
    <main id="main" className="product-app zcash-workspace-page">
      <section className="app-heading workspace-intro">
        <div>
          <p className="eyebrow">Zerant on Zcash · Testnet</p>
          <h1>Pay with a Zcash testnet wallet.</h1>
          <p>Prepare and review a payment in Zerant. Then open its payment link, scan its QR code, or copy the exact details into your testnet wallet. Browser connection is optional.</p>
        </div>
        <span className="pill">{authenticated ? "Zerant account connected" : "Sign-in required"}</span>
      </section>

      {activeSection !== undefined ? (
        <WorkspaceSectionNav
          activeHref={activeSection === "overview" ? "/zcash" : `/zcash/${activeSection}`}
          items={[
            { href: "/zcash", label: "Overview" },
            { href: "/zcash/payments", label: "Payments" },
            { href: "/zcash/invoices", label: "Invoices" },
            { href: "/zcash/address", label: "Address check" },
          ]}
        />
      ) : null}

      <ZcashLiveStatus authenticated={authenticated} network={liveNetwork} onNetworkUpdate={setLiveNetwork} />

      {showAllSections ? <nav className="zcash-shortcuts" aria-label="Zcash tools">
        {activeSection === undefined ? <a href="#zcash-payment-review">Prepare or review a payment</a> : <Link href="/zcash/payments">Prepare or review a payment</Link>}
        {activeSection === undefined ? <a href="#zcash-invoices">Request ZEC</a> : <Link href="/zcash/invoices">Request ZEC</Link>}
        {activeSection === undefined ? <a href="#zcash-address-inspector">Check a Zcash address</a> : <Link href="/zcash/address">Check a Zcash address</Link>}
      </nav> : null}

      {showOverview ? <section className="zcash-wallet-setup" aria-labelledby="zcash-testnet-wallet-title">
        <div className="section-heading">
          <p className="eyebrow">Testnet wallet setup</p>
          <h2 id="zcash-testnet-wallet-title">Bring a Zcash testnet wallet.</h2>
          <p className="muted">Zerant works with a compatible wallet through a standard Zcash payment link. You can also use the exact recipient and amount in a testnet wallet that cannot open the link.</p>
        </div>
        <div className="zcash-wallet-setup-grid">
          <article className="workspace-card">
            <p className="eyebrow">No browser connection</p>
            <h3>Open, scan, or copy</h3>
            <p className="muted">Review the recipient and amount, then use the Zcash payment link or QR code in a compatible testnet wallet. For a simple payment, you can copy both fields manually.</p>
            <Link className="button" href="/zcash/payments">Prepare a payment <span aria-hidden="true">→</span></Link>
          </article>
          <article className="workspace-card">
            <p className="eyebrow">Before you send</p>
            <h3>Get the recipient’s testnet address</h3>
            <p className="muted">A Zerant ID is for credentials and account access. Payments need a Zcash testnet receive address from the person you are paying.</p>
            <Link className="text-link" href="/zcash/address">Check an address <span aria-hidden="true">→</span></Link>
          </article>
        </div>
        <p className="small muted">Wallet installation and payment approval stay outside Zerant. Never enter a recovery phrase, spending key, or wallet password into Zerant.</p>
      </section> : null}

      {showOverview ? <section className="workspace-grid zcash-overview-grid" aria-label="Zcash status">
        <article className="workspace-card workspace-card-primary">
          <div className="workspace-card-top">
            <div><p className="eyebrow">Network</p><h2>{readiness.title}</h2></div>
            <span className="status-dot" aria-hidden="true" />
          </div>
          <p>{readiness.body}</p>
          <div className={networkActionsReady ? "workspace-state ready" : "workspace-state"}><span />{readiness.label}</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Identity and payments</p>
          <h2>Your Zerant ID is not a Zcash address.</h2>
          <p className="muted">Your passkey opens your Zerant account. A Zcash wallet approves payments. Zerant does not use wallet balances or history as your identity.</p>
          <Link className="text-link" href="/account">Manage account access <span aria-hidden="true">→</span></Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Payment tracking</p>
          <h2>Review first. Approve in your wallet.</h2>
          <p className="muted">If your wallet submits the payment, save its transaction ID in Zerant. Zerant can observe that exact txid on trusted Zcash testnet infrastructure. Network visibility does not independently verify a shielded recipient or amount.</p>
          <div className="zcash-lifecycle" aria-label="Available Zcash payment states">
            <span>Prepared</span><span>Submitted</span><span>Seen</span><span>Mined · depth</span>
          </div>
        </article>
      </section> : null}

      {activeSection === "overview" ? (
        <section className="workspace-route-cards" aria-label="Zcash workspace sections">
          <Link href="/zcash/payments"><strong>Payments</strong><span>Review a payment and hand it to a compatible wallet.</span></Link>
          <Link href="/zcash/invoices"><strong>Invoices</strong><span>Create a shareable request for a specific amount.</span></Link>
          <Link href="/zcash/address"><strong>Address check</strong><span>Inspect a destination before using it in a request.</span></Link>
        </section>
      ) : null}

      {!authenticated ? (
        <section className="workspace-wallet">
          <div className="workspace-wallet-heading">
            <p className="eyebrow">Account required</p>
            <h2>Sign in before using private Zcash actions.</h2>
            <p className="muted">Passkey access is available even when wallet sign-in is not.</p>
          </div>
          <Link className="button" href="/vault">Choose a sign-in method →</Link>
        </section>
      ) : null}

      {showSection("payments") ? <ZcashPaymentRequestReview
        enabled={authenticated}
        observationAvailable={Boolean(liveNetwork?.network_actions_enabled)}
      /> : null}

      {showSection("invoices") ? <ZcashInvoiceManager enabled={authenticated} /> : null}

      {authenticated && showSection("wallet") ? (
        <section id="zcash-wallet-actions" className="workspace-wallet" aria-label="Zcash wallet access">
          <div className="workspace-wallet-heading">
            <p className="eyebrow">Optional direct connection</p>
            <h2>Connect a wallet only if it supports this network.</h2>
            <p className="muted">Zerant uses Zcash testnet. The Noir extension from the Chrome Store is mainnet and cannot pay a testnet request. A separate Testnet Noir build is available from Noir. You can use a compatible wallet through <Link href="/zcash/payments">payment review</Link> without connecting it to Zerant.</p>
          </div>
          <ZcashConnect purpose="connection" />
          <p className="small muted wallet-compatibility-note">A Zcash address identifies a payment destination. It does not connect a wallet or give Zerant spending authority.</p>
        </section>
      ) : null}

      {showSection("address") ? <ZcashAddressInspector enabled={authenticated} /> : null}
    </main>
  );
}
