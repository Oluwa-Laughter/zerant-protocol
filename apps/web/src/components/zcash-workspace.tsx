import Link from "next/link";
import { ZcashAddressInspector } from "@/components/zcash-address-inspector";
import { ZcashConnect } from "@/components/zcash-connect";
import { ZcashPaymentRequestReview } from "@/components/zcash-payment-request-review";
import { ZcashLiveStatus } from "@/components/zcash-live-status";
import type { WorkspaceSession, WorkspaceZcash, WorkspaceZcashNetwork } from "@/components/product-workspace";

function readinessCopy(network: WorkspaceZcashNetwork, zcash: WorkspaceZcash) {
  if (network?.state === "ready") return { label: "Ready", title: "Zcash network ready.", body: "Zerant has a fresh network view for supported Zcash actions." };
  if (network?.state === "syncing") return { label: "Syncing", title: "Zcash network syncing.", body: "Network-dependent actions stay protected while Zerant catches up." };
  if (network?.state === "degraded") return { label: "Protected mode", title: "Zcash network temporarily unavailable.", body: "Zerant keeps network-dependent actions disabled until a fresh check succeeds." };
  if (zcash) return { label: "Available", title: "Zcash services available.", body: "Zerant can perform supported Zcash validation and wallet actions." };
  return { label: "Unavailable", title: "Zcash network connection unavailable.", body: "Private credentials still work, but Zcash network actions are unavailable." };
}

export function ZcashWorkspace({ session, zcash, network }: { session: WorkspaceSession; zcash: WorkspaceZcash; network: WorkspaceZcashNetwork }) {
  const authenticated = Boolean(session?.authenticated);
  const readiness = readinessCopy(network, zcash);

  return (
    <main id="main" className="product-app zcash-workspace-page">
      <section className="app-heading workspace-intro">
        <div>
          <p className="eyebrow">Zerant on Zcash · Testnet</p>
          <h1>Prepare a Zcash payment. Keep control in your wallet.</h1>
          <p>Zerant is built for the Zcash ecosystem. It prepares and tracks a specific payment request while a compatible Zcash wallet holds funds and approves the transaction.</p>
        </div>
        <span className="pill">{authenticated ? "Zerant account connected" : "Sign-in required"}</span>
      </section>

      <ZcashLiveStatus authenticated={authenticated} networkState={network?.state ?? null} />

      <nav className="zcash-shortcuts" aria-label="Zcash tools">
        <a href="#zcash-payment-review">Prepare or review a payment</a>
        <a href="#zcash-address-inspector">Check a Zcash address</a>
        {authenticated ? <a href="#zcash-wallet-actions">Connect a testnet wallet</a> : null}
      </nav>

      <section className="zcash-wallet-setup" aria-labelledby="zcash-testnet-wallet-title">
        <div className="section-heading">
          <p className="eyebrow">Testnet wallet setup</p>
          <h2 id="zcash-testnet-wallet-title">Use a wallet that is actually on Zcash testnet.</h2>
          <p className="muted">The Noir Wallet from the Chrome Web Store is mainnet. For Zerant testnet, use Noir’s separate official testnet extension build, or use a compatible testnet wallet through the reviewed ZIP-321 payment link.</p>
        </div>
        <div className="zcash-wallet-setup-grid">
          <article className="workspace-card">
            <p className="eyebrow">Direct connection</p>
            <h3>Testnet Noir Wallet</h3>
            <p className="muted">Noir publishes a separate testnet extension on its official GitHub Releases page. Download the asset ending in <code>-testnet.zip</code>, unzip it, then load the extracted extension from Chrome’s Extensions page in Developer mode. The installed extension should identify itself as <strong>[Testnet] Noir Wallet</strong>.</p>
            <a className="button" href="https://github.com/NoirWallet/noir-wallet-sdk/releases" target="_blank" rel="noreferrer">Open official Noir releases <span aria-hidden="true">↗</span></a>
          </article>
          <article className="workspace-card">
            <p className="eyebrow">Portable payment</p>
            <h3>ZIP-321 testnet wallet</h3>
            <p className="muted">A wallet does not need a Zerant browser connection to pay a reviewed request. Zerant can produce the canonical <code>zcash:</code> payment link; wallets such as Zingo document testnet wallets and ZIP-321 URI handling.</p>
            <a className="text-link" href="https://github.com/zingolabs/zingo-pc" target="_blank" rel="noreferrer">Review Zingo PC support <span aria-hidden="true">↗</span></a>
          </article>
        </div>
        <p className="small muted">Wallet installation and payment approval stay outside Zerant. Never enter a recovery phrase, spending key, or wallet password into Zerant.</p>
      </section>

      <section className="workspace-grid zcash-overview-grid" aria-label="Zcash status">
        <article className="workspace-card workspace-card-primary">
          <div className="workspace-card-top">
            <div><p className="eyebrow">Network</p><h2>{readiness.title}</h2></div>
            <span className="status-dot" aria-hidden="true" />
          </div>
          <p>{readiness.body}</p>
          <div className={network?.network_actions_enabled || zcash ? "workspace-state ready" : "workspace-state"}><span />{readiness.label}</div>
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
          <p className="muted">Zerant saves the transaction ID your wallet returns and can observe that exact txid on trusted Zcash testnet infrastructure. Mempool visibility and confirmation depth are network facts; they do not independently reveal or verify a shielded recipient or amount.</p>
          <div className="zcash-lifecycle" aria-label="Available Zcash payment states">
            <span>Prepared</span><span>Submitted</span><span>Seen</span><span>Mined · depth</span>
          </div>
        </article>
      </section>

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

      <ZcashPaymentRequestReview
        enabled={authenticated}
        observationAvailable={Boolean(network?.network_actions_enabled)}
      />

      {authenticated ? (
        <section id="zcash-wallet-actions" className="workspace-wallet" aria-label="Zcash wallet access">
          <div className="workspace-wallet-heading">
            <p className="eyebrow">Optional direct connection</p>
            <h2>Connect a wallet only if it supports this network.</h2>
            <p className="muted">Zerant uses Zcash testnet. The Noir extension from the Chrome Store is mainnet and cannot pay a testnet request. A separate Testnet Noir build is available from Noir. A compatible wallet can also use the reviewed payment link above without connecting to Zerant.</p>
          </div>
          <ZcashConnect purpose="connection" />
          <p className="small muted wallet-compatibility-note">Pasting a wallet address does not connect a wallet. A destination address belongs in a payment request. Connecting never approves spending.</p>
        </section>
      ) : null}

      <ZcashAddressInspector enabled={authenticated} />
    </main>
  );
}
