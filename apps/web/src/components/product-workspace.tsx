import Link from "next/link";
import { ZcashPaymentRequestReview } from "@/components/zcash-payment-request-review";
import { ZcashAddressInspector } from "@/components/zcash-address-inspector";

export type WorkspaceSession = {
  authenticated: boolean;
  identity: string;
  scopes: string[];
} | null;

export type WorkspaceZcash = {
  capabilities: string[];
  receipt_verification: boolean;
  sendmany_advertised: boolean;
  sendfromaccount_advertised: boolean;
  pczt_complete: boolean;
  wallet: string;
  chain_height: number;
} | null;

export function ProductWorkspace({
  session,
  zcash,
}: {
  session: WorkspaceSession;
  zcash: WorkspaceZcash;
}) {
  const authenticated = Boolean(session?.authenticated);

  return (
    <main id="main" className="product-app">
      <section className="app-heading workspace-intro">
        <div>
          <p className="eyebrow">Zerant workspace</p>
          <h1>Your private trust workspace.</h1>
          <p>
            Zerant helps you build portable trust, prove what matters, and keep unrelated identity and wallet information private.
          </p>
        </div>
        <span className="pill">{authenticated ? "Zcash authenticated" : "Server-backed"}</span>
      </section>

      <section className="workspace-explainer">
        <div>
          <span className="eyebrow">What happens here</span>
          <h2>Connect. Receive. Prove.</h2>
        </div>
        <ol>
          <li><strong>Connect your Zcash identity</strong><span>Create a private Zerant identity without publishing your payment address or wallet history.</span></li>
          <li><strong>Receive trusted credentials</strong><span>Collect proof of contributions, roles, memberships, achievements or eligibility from trusted issuers.</span></li>
          <li><strong>Approve narrow verification requests</strong><span>Zerant returns only the bounded result after native trust, revocation, audience and replay checks pass.</span></li>
        </ol>
      </section>

      <section className="workspace-grid" aria-label="Zerant product workspace">
        <article className="workspace-card workspace-card-primary">
          <div className="workspace-card-top">
            <div><p className="eyebrow">Credential vault</p><h2>Your private evidence</h2></div>
            <span className="status-dot" aria-hidden="true" />
          </div>
          <p>
            {authenticated
              ? "Your private credentials are ready."
              : "Connect your Zcash identity to start receiving and proving trusted credentials."}
          </p>
          <p className="small muted">
            Your credentials follow your Zerant account instead of being trapped in one browser.
          </p>
          <Link className="button" href="/vault">
            {authenticated ? "Open credential vault" : "Connect Zcash identity"} <span aria-hidden="true">→</span>
          </Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Private identity</p>
          <h2>{authenticated ? "Identity connected." : "Not connected."}</h2>
          <p className="muted">
            {authenticated
              ? "Your Zerant identity is connected without exposing your payment address."
              : "Connect a private Zcash identity that remains separate from the keys controlling your funds."}
          </p>
          <div className={authenticated ? "workspace-state ready" : "workspace-state"}>
            <span />{authenticated ? "Private identity connected" : "Connection required"}
          </div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Verification requests</p>
          <h2>No request waiting for you.</h2>
          <p className="muted">
            Incoming requests will show requester origin, purpose, requested conditions and
            disclosure boundaries before approval.
          </p>
          <Link className="text-link" href="/requests">Review requests <span aria-hidden="true">→</span></Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Zcash readiness</p>
          <h2>{zcash ? "Zcash services ready." : "Zcash services unavailable."}</h2>
          <p className="muted">
            {zcash
              ? "Zerant can validate Zcash activity and prepare supported privacy-preserving actions."
              : "Zcash-powered verification and payment review are temporarily unavailable."}
          </p>
          <div className={zcash ? "workspace-state ready" : "workspace-state"}>
            <span />{zcash ? "Zcash ready" : "Unavailable"}
          </div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Shared approvals</p>
          <h2>Require more than one person when it matters.</h2>
          <p className="muted">
            Teams and organizations can use shared approval policies for sensitive treasury actions instead of relying on one person alone.
          </p>
          <div className="workspace-state"><span />Shared approval support</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">For organizations</p>
          <h2>Issue trust. Verify privately.</h2>
          <p className="muted">
            Create trusted credentials for people, or request a narrow proof without collecting their whole identity profile.
          </p>
          <div className="workspace-actions">
            <Link className="text-link" href="/issuer">Issue credentials →</Link>
            <Link className="text-link" href="/verifier">Request proof →</Link>
          </div>
        </article>
      </section>

      <ZcashAddressInspector enabled={authenticated} />
      <ZcashPaymentRequestReview enabled={authenticated} />
    </main>
  );
}
