import Link from "next/link";
import { ZcashAddressInspector } from "@/components/zcash-address-inspector";
import { ZcashConnect } from "@/components/zcash-connect";
import { ZcashPaymentRequestReview } from "@/components/zcash-payment-request-review";
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
          <p className="eyebrow">Zcash workspace</p>
          <h1>Private payments, separate from your Zerant ID.</h1>
          <p>Your passkey opens Zerant. A Zcash wallet holds funds and approves payments. Connect one only when you choose to pay.</p>
        </div>
        <span className="pill">{authenticated ? "Zerant account connected" : "Sign-in required"}</span>
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
          <p className="eyebrow">Privacy boundary</p>
          <h2>Your wallet stays separate from Zerant identity.</h2>
          <p className="muted">Zerant does not use your balance, payment address, or transaction history as your account identity.</p>
          <Link className="text-link" href="/account">Manage account access <span aria-hidden="true">→</span></Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Payment tracking</p>
          <h2>Review first. Approve in your wallet.</h2>
          <p className="muted">Zerant can save a prepared payment and the transaction ID your wallet returns. Testnet settlement verification is not available yet, so a submitted payment stays pending here.</p>
          <div className="zcash-lifecycle" aria-label="Available Zcash payment states">
            <span>Prepared</span><span>Submitted · pending</span>
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
      ) : (
        <section id="zcash-wallet-actions" className="workspace-wallet" aria-label="Zcash wallet access">
          <div className="workspace-wallet-heading">
            <p className="eyebrow">Wallet access</p>
            <h2>Use your own Zcash wallet to pay.</h2>
            <p className="muted">Zerant does not contain a wallet or hold your funds. On desktop Chrome with the Testnet Noir extension, choose Noir and approve the site in its popup. Connecting does not approve a payment.</p>
          </div>
          <ZcashConnect purpose="connection" />
          <p className="small muted wallet-compatibility-note">Pasting a wallet address does not connect a wallet. A destination address belongs in the payment request below. If Noir is unavailable, you can review and copy a payment request for a compatible wallet app.</p>
        </section>
      )}

      <ZcashPaymentRequestReview enabled={authenticated} />
      <ZcashAddressInspector enabled={authenticated} />
    </main>
  );
}
