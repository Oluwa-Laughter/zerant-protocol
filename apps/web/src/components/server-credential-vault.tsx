"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { ZcashConnect } from "@/components/zcash-connect";

export type ServerVaultSession = {
  authenticated: boolean;
  identity: string;
  scopes: string[];
};

export type ServerVaultCredential = {
  id: string;
  credential: unknown;
  created_at: string;
  updated_at: string;
};

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
  const [input, setInput] = useState("");
  const [status, setStatus] = useState(
    backendAvailable
      ? initialSession
        ? "Credential vault loaded from Zerant's encrypted server store."
        : "Connect your Zcash identity to access the server credential vault."
      : "The Zerant Rust API is not configured for this deployment.",
  );

  async function store() {
    let credential: unknown;
    try {
      credential = JSON.parse(input);
    } catch {
      setStatus("Credential must be valid JSON.");
      return;
    }

    const response = await fetch("/api/zerant/credentials", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ credential }),
    });
    if (!response.ok) {
      setStatus("Credential could not be stored.");
      return;
    }

    const stored = (await response.json()) as ServerVaultCredential;
    setCredentials((current) => [stored, ...current.filter((item) => item.id !== stored.id)]);
    setInput("");
    setStatus("Credential encrypted and stored server-side.");
  }

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
          <p className="eyebrow">Server credential vault</p>
          <h1>The credential service is not configured.</h1>
          <p>
            Zerant no longer falls back to browser storage. Configure the Rust API and PostgreSQL
            backend before credentials can be stored or retrieved.
          </p>
        </section>
      </main>
    );
  }

  if (!session) {
    return (
      <main id="main" className="vault-page">
        <section className="vault-hero">
          <p className="eyebrow">Server credential vault</p>
          <h1>Your credentials belong to your Zerant account, not one browser.</h1>
          <p>
            Durable credential state is encrypted by the Rust service and persisted in
            PostgreSQL. Authentication is Zcash-native through ZecAuth.
          </p>
        </section>
        <ZcashConnect onConnected={() => window.location.reload()} />
      </main>
    );
  }

  return (
    <main id="main" className="vault-page">
      <section className="vault-hero">
        <p className="eyebrow">Server credential vault</p>
        <h1>Your private trust records, available across devices.</h1>
        <p>
          Credentials are envelope-encrypted by the Zerant Rust service and persisted in
          PostgreSQL. Browser storage is not the source of truth.
        </p>
      </section>

      <section className="vault-grid">
        <article className="vault-card">
          <div className="vault-card-heading">
            <div>
              <p className="eyebrow">Zcash identity</p>
              <h2>Authenticated</h2>
            </div>
            <span className="vault-state unlocked">ZecAuth</span>
          </div>
          <p className="small muted mono">{session.identity}</p>
          <p className="small muted">
            This is a purpose-specific Zcash authentication identity, not a payment address.
          </p>
          <div className="vault-actions">
            <Button variant="secondary" onClick={logout}>Sign out</Button>
          </div>
        </article>

        <article className="vault-card">
          <div className="vault-card-heading">
            <div>
              <p className="eyebrow">Credentials</p>
              <h2>{credentials.length} stored record{credentials.length === 1 ? "" : "s"}</h2>
            </div>
            <span className="pill">Envelope-encrypted server-side</span>
          </div>

          <div className="vault-form">
            <label htmlFor="credential-json">Import a credential JSON</label>
            <textarea
              id="credential-json"
              value={input}
              onChange={(event) => setInput(event.target.value)}
              spellCheck={false}
              rows={8}
              placeholder="Paste a real credential JSON"
            />
            <Button disabled={!input.trim()} onClick={store}>Store credential</Button>
          </div>

          <div className="server-record-list">
            {credentials.length ? credentials.map((item) => (
              <article className="server-record" key={item.id}>
                <div>
                  <span className="mono">{item.id}</span>
                  <span className="small muted">
                    Updated {new Date(item.updated_at).toLocaleString()}
                  </span>
                </div>
                <details>
                  <summary>View decrypted record</summary>
                  <pre className="vault-preview">{JSON.stringify(item.credential, null, 2)}</pre>
                </details>
                <Button variant="secondary" onClick={() => remove(item.id)}>Remove</Button>
              </article>
            )) : <p className="muted">No credentials are stored for this account yet.</p>}
          </div>
        </article>
      </section>

      <section className="vault-security">
        <div>
          <p className="eyebrow">Trust boundary</p>
          <h2>Server-side does not mean plaintext.</h2>
        </div>
        <div className="vault-security-list">
          <p><strong>Persistence:</strong> PostgreSQL is the source of truth. Every credential is encrypted with a fresh data key and that key is wrapped by the server KEK.</p>
          <p><strong>Authentication:</strong> ZecAuth proves control of a purpose-specific RedPallas key without exposing Zcash spending authority.</p>
          <p><strong>Wallet operations:</strong> Z3, Zallet, PCZT and FROST remain native/server integrations and are never delegated to browser JavaScript.</p>
        </div>
      </section>

      <p className="vault-status neutral" role="status">{status}</p>
    </main>
  );
}
