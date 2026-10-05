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
          <h1>Use Zcash without turning your wallet into your identity.</h1>
          <p>Prepare private payments, validate destinations, and connect a wallet only for the action you choose.</p>
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
          <p className="eyebrow">Payment lifecycle</p>
          <h2>Submission is not settlement.</h2>
          <p className="muted">After a wallet submits a transaction, Zerant keeps it pending until supported network evidence can confirm settlement.</p>
          <div className="zcash-lifecycle" aria-label="Zcash payment lifecycle">
            <span>Prepared</span><span>Submitted</span><span>Observed</span><span>Confirmed</span>
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
            <h2>Connect only when you need a wallet action.</h2>
            <p className="muted">The wallet remains responsible for approval and spending authority. Zerant never treats connection as consent to send funds.</p>
          </div>
          <ZcashConnect purpose="connection" />
        </section>
      )}

      <ZcashPaymentRequestReview enabled={authenticated} />
      <ZcashAddressInspector enabled={authenticated} />
    </main>
  );
}
