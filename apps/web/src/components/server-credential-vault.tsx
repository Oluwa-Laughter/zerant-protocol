"use client";

import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { PasskeyAccess } from "@/components/passkey-access";

export type ServerVaultSession = {
  authenticated: boolean;
  identity: string;
  zerant_id: string;
  scopes: string[];
};

export type ServerVaultCredential = {
  id: string;
  credential: unknown;
  revoked: boolean;
  expired: boolean;
  created_at: string;
  updated_at: string;
};

type CredentialFilter = "all" | "active" | "expired" | "revoked";

type PrivateCredential = {
  type: "zerant.private-credential";
  issuer: string;
  credential_id: string;
  credential_schema_id?: string | null;
  credential_name?: string | null;
  claim_type: string;
  value: string;
  context: string;
  issued_at: number;
  expires_at: number;
};

function asPrivateCredential(value: unknown): PrivateCredential | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  if (
    record.type !== "zerant.private-credential" ||
    typeof record.issuer !== "string" ||
    typeof record.credential_id !== "string" ||
    typeof record.claim_type !== "string" ||
    typeof record.value !== "string" ||
    typeof record.context !== "string" ||
    typeof record.issued_at !== "number" ||
    typeof record.expires_at !== "number"
  ) {
    return null;
  }
  return record as PrivateCredential;
}

export function credentialLifecycleState(
  item: Pick<ServerVaultCredential, "revoked" | "expired">,
): Exclude<CredentialFilter, "all"> {
  if (item.revoked) return "revoked";
  if (item.expired) return "expired";
  return "active";
}

export function ServerCredentialVault({
  initialSession,
  initialCredentials,
  backendAvailable,
}: {
  initialSession: ServerVaultSession | null;
  initialCredentials: ServerVaultCredential[];
  backendAvailable: boolean;
}) {
  const [session, setSession] = useState<ServerVaultSession | null>(initialSession);
  const [credentials, setCredentials] = useState<ServerVaultCredential[]>(initialCredentials);
  const [filter, setFilter] = useState<CredentialFilter>("all");
  const [pendingRemoval, setPendingRemoval] = useState<string | null>(null);
  const [status, setStatus] = useState(
    backendAvailable
      ? initialSession
        ? "Your private credentials are ready."
        : "Use a passkey to open your Zerant account."
      : "Zerant is temporarily unavailable.",
  );


  async function remove(id: string) {
    const response = await fetch("/api/zerant/credentials/" + encodeURIComponent(id), {
      method: "DELETE",
      credentials: "same-origin",
    });
    if (!response.ok) {
      setStatus("Credential could not be removed.");
      return;
    }
    setCredentials((current) => current.filter((item) => item.id !== id));
    setPendingRemoval(null);
    setStatus("Credential removed from your Zerant account.");
  }

  async function logout() {
    await fetch("/api/zerant/session", {
      method: "DELETE",
      credentials: "same-origin",
    });
    setSession(null);
    setCredentials([]);
    setStatus("Signed out.");
  }

  const credentialRows = useMemo(() => credentials.map((item) => {
    const credential = asPrivateCredential(item.credential);
    const state = credentialLifecycleState(item);
    return { item, credential, state };
  }), [credentials]);

  const credentialCounts = useMemo(() => ({
    active: credentialRows.filter((row) => row.state === "active").length,
    expired: credentialRows.filter((row) => row.state === "expired").length,
    revoked: credentialRows.filter((row) => row.state === "revoked").length,
  }), [credentialRows]);

  const visibleCredentialRows = filter === "all"
    ? credentialRows
    : credentialRows.filter((row) => row.state === filter);

  if (!backendAvailable) {
    return (
      <main id="main" className="vault-page">
        <section className="vault-hero">
          <p className="eyebrow">Private credentials</p>
          <h1>Zerant is temporarily unavailable.</h1>
          <p>Your credentials will never be silently moved into an insecure fallback.</p>
        </section>
      </main>
    );
  }

  if (!session) {
    return (
      <main id="main" className="vault-page">
        <section className="vault-hero">
          <p className="eyebrow">Private credentials</p>
          <h1>Welcome to Zerant.</h1>
          <p>
            Access your private trust workspace with a passkey. Your Zerant account
            stays separate from your Zcash wallet and payment history.
          </p>
        </section>
        <section className="account-access-options" aria-label="Choose how to enter Zerant">
          <PasskeyAccess onConnected={() => window.location.reload()} />
        </section>
      </main>
    );
  }

  return (
    <main id="main" className="vault-page">
      <section className="vault-hero">
        <p className="eyebrow">Your Zerant account</p>
        <h1>Your private credentials.</h1>
        <p>
          Credentials you receive stay private until you choose to use them for a specific proof.
        </p>
      </section>

      <section className="vault-grid">
        <article className="vault-card zerant-id-card">
          <div className="vault-card-heading">
            <div>
              <p className="eyebrow">Your Zerant ID</p>
              <h2>Share this to receive credentials.</h2>
            </div>
            <span className="vault-state unlocked">Connected</span>
          </div>
          <p className="zerant-id-value mono">{session.zerant_id}</p>
          <p className="small muted">
            Your Zerant ID is for receiving trust credentials. It is not your payment address and
            does not reveal your wallet balance or transaction history.
          </p>
          <div className="vault-actions">
            <Button
              variant="secondary"
              onClick={() => void navigator.clipboard?.writeText(session.zerant_id)}
            >
              Copy Zerant ID
            </Button>
            <Button variant="secondary" onClick={logout}>Sign out</Button>
          </div>
        </article>

        <article className="vault-card">
          <div className="vault-card-heading">
            <div>
              <p className="eyebrow">Credentials</p>
              <h2>{credentials.length} private credential{credentials.length === 1 ? "" : "s"}</h2>
            </div>
            <span className="pill">Private</span>
          </div>

          <div className="credential-vault-summary" aria-label="Credential status summary">
            <button type="button" className={filter === "active" ? "credential-summary active" : "credential-summary"} onClick={() => setFilter(filter === "active" ? "all" : "active")}><strong>{credentialCounts.active}</strong><span>Active</span></button>
            <button type="button" className={filter === "expired" ? "credential-summary active" : "credential-summary"} onClick={() => setFilter(filter === "expired" ? "all" : "expired")}><strong>{credentialCounts.expired}</strong><span>Expired</span></button>
            <button type="button" className={filter === "revoked" ? "credential-summary active" : "credential-summary"} onClick={() => setFilter(filter === "revoked" ? "all" : "revoked")}><strong>{credentialCounts.revoked}</strong><span>Revoked</span></button>
          </div>

          {filter !== "all" ? <div className="credential-filter-note"><span>Showing {filter} credentials</span><button type="button" onClick={() => setFilter("all")}>Show all</button></div> : null}

          <div className="credential-card-list">
            {visibleCredentialRows.length ? visibleCredentialRows.map(({ item, credential, state }) => {
              if (!credential) {
                return (
                  <article className={`credential-card state-${state}`} key={item.id}>
                    <div className="credential-card-top">
                      <div>
                        <span className="eyebrow">Private credential</span>
                        <h3>Stored credential</h3>
                      </div>
                    </div>
                    <p className="small muted">
                      Added {new Date(item.created_at).toLocaleDateString()}.
                    </p>
                    {pendingRemoval === item.id ? <div className="credential-remove-confirm"><span className="small muted">Remove this credential from your account?</span><div className="vault-actions wrap"><Button variant="secondary" onClick={() => void remove(item.id)}>Confirm remove</Button><Button variant="secondary" onClick={() => setPendingRemoval(null)}>Cancel</Button></div></div> : <Button variant="secondary" onClick={() => setPendingRemoval(item.id)}>Remove</Button>}
                  </article>
                );
              }

              return (
                <article className={`credential-card state-${state}`} key={item.id}>
                  <div className="credential-card-top">
                    <div>
                      <span className="eyebrow">{credential.issuer}</span>
                      <h3>{credential.credential_name ?? credential.claim_type}</h3>
                    </div>
                    <span className={state === "revoked" ? "credential-status revoked" : state === "expired" ? "credential-status expired" : "credential-status active"}>
                      {state === "revoked" ? "Revoked" : state === "expired" ? "Expired" : "Active"}
                    </span>
                  </div>
                  <p className={state === "active" ? "credential-value" : "credential-value revoked-value"}>{credential.value}</p>
                  <div className="credential-detail-row"><span>{credential.context}</span><span>{credential.claim_type}</span></div>
                  <p className={state === "active" ? "small muted" : "small revoked-note"}>
                    {state === "revoked"
                      ? "This credential was revoked by its issuer and cannot be used for new proofs."
                      : state === "expired"
                        ? "This credential expired on " + new Date(credential.expires_at * 1000).toLocaleDateString() + " and cannot be used for new proofs."
                        : "Valid until " + new Date(credential.expires_at * 1000).toLocaleDateString()}
                  </p>
                  {pendingRemoval === item.id ? <div className="credential-remove-confirm"><span className="small muted">Remove this credential from your account?</span><div className="vault-actions wrap"><Button variant="secondary" onClick={() => void remove(item.id)}>Confirm remove</Button><Button variant="secondary" onClick={() => setPendingRemoval(null)}>Cancel</Button></div></div> : <Button variant="secondary" onClick={() => setPendingRemoval(item.id)}>Remove</Button>}
                </article>
              );
            }) : (
              <div className="empty-credentials">
                <h3>{credentials.length ? `No ${filter} credentials.` : "No credentials yet."}</h3>
                <p className="muted">
                  {credentials.length ? "Choose another filter to review the rest of your vault." : "Share your Zerant ID with a trusted organization when you are ready to receive one."}
                </p>
              </div>
            )}
          </div>
        </article>
      </section>

      <section className="vault-security">
        <div>
          <p className="eyebrow">Your privacy</p>
          <h2>You decide what leaves your account.</h2>
        </div>
        <div className="vault-security-list">
          <p><strong>Credentials stay private:</strong> receiving a credential does not make it public.</p>
          <p><strong>Your wallet stays separate:</strong> your Zerant identity does not expose your payment address, balance or history.</p>
          <p><strong>Proofs require consent:</strong> a verifier receives only the approved result for a specific request.</p>
        </div>
      </section>

      <p className="vault-status neutral" role="status">{status}</p>
    </main>
  );
}
