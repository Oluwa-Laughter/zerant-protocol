import Link from "next/link";

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

export type WorkspaceZcashNetwork = {
  configured: boolean;
  network: string;
  state: "ready" | "syncing" | "degraded" | "not_configured";
  network_actions_enabled: boolean;
  synced: boolean;
  block_height: number | null;
  estimated_height: number | null;
  lag: number | null;
  last_confirmed_at: string | null;
} | null;

export function ProductWorkspace({
  session,
  zcash,
  zcashNetwork,
}: {
  session: WorkspaceSession;
  zcash: WorkspaceZcash;
  zcashNetwork: WorkspaceZcashNetwork;
}) {
  const authenticated = Boolean(session?.authenticated);

  return (
    <main id="main" className="product-app">
      <section className="app-heading workspace-intro">
        <div>
          <p className="eyebrow">Zerant · Zcash testnet</p>
          <h1>Private trust for the Zcash ecosystem.</h1>
          <p>
            Receive credentials, approve specific proofs, and prepare Zcash payments from one workspace. Your Zerant ID and wallet serve different purposes.
          </p>
        </div>
        <span className="pill">{authenticated ? "Signed in" : "Sign-in required"}</span>
      </section>

      <section className="workspace-explainer">
        <div>
          <span className="eyebrow">What happens here</span>
          <h2>Sign in. Receive. Prove. Pay when needed.</h2>
        </div>
        <ol>
          <li><strong>Open your Zerant account</strong><span>Use a passkey. Your Zerant ID is where organizations send credentials, never a payment address.</span></li>
          <li><strong>Receive trusted credentials</strong><span>Collect proof of contributions, roles, memberships, achievements or eligibility from trusted issuers.</span></li>
          <li><strong>Approve narrow verification requests</strong><span>Zerant returns only the bounded result after native trust, revocation, audience and replay checks pass.</span></li>
          <li><strong>Use Zcash testnet for payments</strong><span>Prepare a payment in Zerant, then approve it in a compatible wallet. Submission stays pending until Zerant can verify settlement.</span></li>
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
              ? "Your vault is ready for credentials issued to your Zerant ID."
              : "Sign in to start receiving and proving trusted credentials."}
          </p>
          <p className="small muted">
            Your credentials follow your Zerant account instead of being trapped in one browser.
          </p>
          <Link className="button" href="/vault">
            {authenticated ? "Open credential vault" : "Choose a sign-in method"} <span aria-hidden="true">→</span>
          </Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Private identity</p>
          <h2>{authenticated ? "Identity connected." : "Not connected."}</h2>
          <p className="muted">
            {authenticated
              ? "Your Zerant identity is connected without exposing your payment address."
              : "Sign in with a passkey or compatible wallet. Your Zerant account stays separate from the keys controlling your funds."}
          </p>
          <div className={authenticated ? "workspace-state ready" : "workspace-state"}>
            <span />{authenticated ? "Private identity connected" : "Connection required"}
          </div>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Verification requests</p>
          <h2>Decide what each verifier learns.</h2>
          <p className="muted">
            Review the requester, purpose, issuer and exact claim before you approve. Denying a request shares no proof.
          </p>
          <Link className="text-link" href="/requests">Review requests <span aria-hidden="true">→</span></Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">Zcash testnet</p>
          <h2>
            {zcashNetwork?.state === "ready"
              ? "Zcash network ready."
              : zcashNetwork?.state === "syncing"
                ? "Zcash network syncing."
                : zcashNetwork?.state === "degraded"
                  ? "Zcash network temporarily unavailable."
                  : zcash
                    ? "Zcash services ready."
                    : "Zcash network connection unavailable."}
          </h2>
          <p className="muted">
            {zcashNetwork?.state === "ready"
              ? "Zerant has a fresh network view for supported Zcash actions."
              : zcashNetwork?.state === "syncing"
                ? "Zerant is catching up before network-dependent actions are enabled."
                : zcashNetwork?.state === "degraded"
                  ? "Zerant is protecting network-dependent actions until a fresh network check succeeds. Your credentials and private proofs remain available."
                  : zcash
                    ? "Zerant can validate supported Zcash activity."
                    : "Credentials and private trust remain available while network connectivity is not configured."}
          </p>
          <div
            className={
              zcashNetwork?.network_actions_enabled || zcash
                ? "workspace-state ready"
                : "workspace-state"
            }
          >
            <span />
            {zcashNetwork?.state === "ready"
              ? "Zcash ready"
              : zcashNetwork?.state === "syncing"
                ? "Syncing"
                : zcashNetwork?.state === "degraded"
                  ? "Protected mode"
                  : zcash
                    ? "Zcash ready"
                    : "Not connected"}
          </div>
          <Link className="text-link" href="/zcash">Prepare a Zcash payment <span aria-hidden="true">→</span></Link>
        </article>

        <article className="workspace-card">
          <p className="eyebrow">How Zcash fits</p>
          <h2>Use Zcash without making your wallet your identity.</h2>
          <p className="muted">
            Zcash moves funds. Zerant coordinates the payment request and keeps its saved state tied to your account. Credentials and consent remain private Zerant workflows.
          </p>
          <Link className="text-link" href="/zcash">Explore Zcash payments <span aria-hidden="true">→</span></Link>
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

    </main>
  );
}
