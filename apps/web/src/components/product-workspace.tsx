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
  backendAvailable,
}: {
  session: WorkspaceSession;
  zcash: WorkspaceZcash;
  backendAvailable: boolean;
}) {
  const authenticated = Boolean(session?.authenticated);

  return (
    <main id="main" className="product-app">
      <section className="app-heading workspace-intro">
        <div>
          <p className="eyebrow">Zerant workspace</p>
          <h1>Your private trust workspace.</h1>
          <p>
            Zerant combines Zcash-native authentication, encrypted server credential storage,
            consent-bound verification and native settlement checks without turning wallet data
            into a public identity profile.
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
          <li><strong>Connect a Zcash identity</strong><span>ZecAuth proves control of a purpose-specific authentication key without exposing spending authority.</span></li>
          <li><strong>Receive trusted credentials</strong><span>Credentials are encrypted by the Rust service and persisted in PostgreSQL, not one browser.</span></li>
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
              ? "Your Zcash-authenticated account can access its encrypted server credential vault."
              : "Connect your Zcash identity to access encrypted server-backed credentials."}
          </p>
          <p className="small muted">
            PostgreSQL is the durable source of truth. Browser storage is not used for credentials
            or session tokens.
          </p>
          <Link className="button" href="/vault">
            {authenticated ? "Open credential vault" : "Connect Zcash identity"} <span aria-hidden="true">→</span>
          </Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Zcash identity</p>
          <h2>{authenticated ? "ZecAuth connected." : "Not connected."}</h2>
          <p className="muted">
            {authenticated
              ? "The session is backed by a RedPallas ZecAuth verification key and an HttpOnly server cookie."
              : "Zerant authenticates with a Zcash-specific key that is isolated from payment addresses and spending keys."}
          </p>
          <div className={authenticated ? "workspace-state ready" : "workspace-state"}>
            <span />{authenticated ? "Authenticated server session" : "Authentication required"}
          </div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Verification requests</p>
          <h2>No request waiting for you.</h2>
          <p className="muted">
            Incoming requests will show requester origin, purpose, requested conditions and
            disclosure boundaries before approval.
          </p>
          <div className="workspace-state"><span />Waiting for a real request</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Zcash infrastructure</p>
          <h2>{zcash ? "Z3 / Zallet reachable." : "Native service not connected."}</h2>
          <p className="muted">
            {zcash
              ? "The server discovered " + zcash.capabilities.length + " RPC methods at chain height " + zcash.chain_height + ". PCZT complete: " + (zcash.pczt_complete ? "yes" : "no") + "."
              : "Z3 capability discovery, Zallet wallet state and payment verification stay behind the Rust service."}
          </p>
          <div className={zcash ? "workspace-state ready" : "workspace-state"}>
            <span />{zcash ? "Live native capability state" : "Server-side Zcash boundary required"}
          </div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Shared-control payments</p>
          <h2>PCZT + FROST boundary.</h2>
          <p className="muted">
            Zerant uses PCZT as the review-first transaction workflow when the running Zallet
            advertises the complete pipeline. FROST remains the threshold-signing boundary for
            organizations and shared treasuries; Zerant does not implement custom threshold crypto.
          </p>
          <div className="workspace-state"><span />Capability-gated native workflow</div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Backend</p>
          <h2>{backendAvailable ? "Rust API reachable." : "API not configured."}</h2>
          <p className="muted">
            Sessions, credential encryption, ZecAuth verification, replay state and Zcash RPC
            access belong to the server. The web app is a presentation and consent surface.
          </p>
          <div className={backendAvailable ? "workspace-state ready" : "workspace-state"}>
            <span />{backendAvailable ? "Server source of truth" : "Configure ZERANT_API_ORIGIN"}
          </div>
        </article>
      </section>

      <ZcashAddressInspector enabled={authenticated} />
      <ZcashPaymentRequestReview enabled={authenticated} />
    </main>
  );
}
