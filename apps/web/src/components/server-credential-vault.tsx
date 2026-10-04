"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { ZcashConnect } from "@/components/zcash-connect";

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
  created_at: string;
  updated_at: string;
};

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
  const [status, setStatus] = useState(
    backendAvailable
      ? initialSession
        ? "Your private credentials are ready."
        : "Connect your Zcash identity to open your Zerant account."
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
    setStatus("Credential removed.");
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
          <h1>Build trust without building a public profile.</h1>
          <p>
            Connect your Zcash identity to receive trusted credentials and prove only what a
            verifier actually needs to know.
          </p>
        </section>
        <ZcashConnect onConnected={() => window.location.reload()} />
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

          <div className="credential-card-list">
            {credentials.length ? credentials.map((item) => {
              const credential = asPrivateCredential(item.credential);
              if (!credential) {
                return (
                  <article className={"credential-card" + (item.revoked ? " revoked" : "")} key={item.id}>
                    <div className="credential-card-top">
                      <div>
                        <span className="eyebrow">Private credential</span>
                        <h3>Stored credential</h3>
                      </div>
                    </div>
                    <p className="small muted">
                      Added {new Date(item.created_at).toLocaleDateString()}.
                    </p>
                    <Button variant="secondary" onClick={() => remove(item.id)}>Remove</Button>
                  </article>
                );
              }

              return (
                <article className="credential-card" key={item.id}>
                  <div className="credential-card-top">
                    <div>
                      <span className="eyebrow">{credential.issuer}</span>
                      <h3>{credential.credential_name ?? credential.claim_type}</h3>
                    </div>
                    <span className={item.revoked ? "credential-status revoked" : "pill"}>
                      {item.revoked ? "Revoked" : credential.context}
                    </span>
                  </div>
                  <p className={item.revoked ? "credential-value revoked-value" : "credential-value"}>{credential.value}</p>
                  <p className={item.revoked ? "small revoked-note" : "small muted"}>
                    {item.revoked
                      ? "This credential was revoked by its issuer and cannot be used for new proofs."
                      : "Valid until " + new Date(credential.expires_at * 1000).toLocaleDateString()}
                  </p>
                  <Button variant="secondary" onClick={() => remove(item.id)}>Remove</Button>
                </article>
              );
            }) : (
              <div className="empty-credentials">
                <h3>No credentials yet.</h3>
                <p className="muted">
                  Share your Zerant ID with a trusted organization when you are ready to receive one.
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
