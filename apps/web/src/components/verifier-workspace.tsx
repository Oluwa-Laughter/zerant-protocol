"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

export type VerifierProfile = {
  display_name: string;
  origin: string;
  created_at: string;
};

export type TrustedCredentialSchema = {
  id: string;
  issuer_id: string;
  issuer_name: string;
  display_name: string;
  description: string;
  claim_type: string;
  context: string;
  default_expiry_days: number;
  active: boolean;
  created_at: string;
};

export type TrustedIssuerOption = {
  display_name: string;
  issuer_id: string;
  schemas: TrustedCredentialSchema[];
};



export type VerifierKeyView = {
  active: boolean;
  compromised: boolean;
  valid_from: string;
  retired_at: string | null;
};

export type VerificationRequestItem = {
  id: string;
  holder_zerant_id: string;
  purpose: string;
  credential_schema_id: string | null;
  credential_name: string | null;
  claim_type: string;
  context: string;
  status: string;
  verified: boolean;
  created_at: string;
  expires_at: string;
};

export function VerifierWorkspace({
  authenticated,
  backendAvailable,
  initialProfile,
  issuers,
  initialRequests,
  initialKeys,
}: {
  authenticated: boolean;
  backendAvailable: boolean;
  initialProfile: VerifierProfile | null;
  issuers: TrustedIssuerOption[];
  initialRequests: VerificationRequestItem[];
  initialKeys: VerifierKeyView[];
}) {
  const availableSchemas = useMemo(
    () =>
      issuers.flatMap((issuer) =>
        issuer.schemas
          .filter((schema) => schema.active)
          .map((schema) => ({
            ...schema,
            issuer_name: issuer.display_name,
            issuer_id: issuer.issuer_id,
          })),
      ),
    [issuers],
  );

  const [profile, setProfile] = useState(initialProfile);
  const [requests, setRequests] = useState(initialRequests);
  const [keys, setKeys] = useState<VerifierKeyView[]>(initialKeys);
  const [displayName, setDisplayName] = useState("");
  const [origin, setOrigin] = useState("");
  const [holderId, setHolderId] = useState("");
  const [purpose, setPurpose] = useState("");
  const [schemaId, setSchemaId] = useState(availableSchemas[0]?.id ?? "");
  const [status, setStatus] = useState("");

  const selectedSchema = availableSchemas.find((schema) => schema.id === schemaId);

  async function activate() {
    const response = await fetch("/api/zerant/verifier", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ display_name: displayName, origin }),
    });
    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus(
        response.status === 409
          ? "This website or account is already registered."
          : "Verifier profile could not be created. Use the exact https:// website origin.",
      );
      return;
    }
    setProfile((await response.json()) as VerifierProfile);
    setStatus("Verifier profile is active.");
  }

  async function rotateVerifierKey(compromiseCurrent: boolean) {
    const response = await fetch("/api/zerant/verifier/keys/rotate", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ compromise_current: compromiseCurrent }),
    });

    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus("Verification security key could not be replaced.");
      return;
    }

    const created = (await response.json()) as VerifierKeyView;
    const now = new Date().toISOString();
    setKeys((current) => [
      created,
      ...current.map((item) =>
        item.active
          ? {
              ...item,
              active: false,
              compromised: compromiseCurrent || item.compromised,
              retired_at: now,
            }
          : item,
      ),
    ]);

    if (compromiseCurrent) {
      const requestsResponse = await fetch("/api/zerant/verifier/requests", {
        credentials: "same-origin",
        cache: "no-store",
      });
      if (requestsResponse.ok) {
        setRequests((await requestsResponse.json()) as VerificationRequestItem[]);
      }
    }

    setStatus(
      compromiseCurrent
        ? "Compromised key replaced. Pending requests signed by it were expired."
        : "Security key rotated. Existing short-lived requests can finish normally.",
    );
  }

  async function createRequest() {
    if (!selectedSchema) {
      setStatus("Choose a trusted credential type first.");
      return;
    }

    const response = await fetch("/api/zerant/verifier/requests", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        holder_zerant_id: holderId,
        purpose,
        credential_schema_id: selectedSchema.id,
        accepted_issuer_ids: [selectedSchema.issuer_id],
      }),
    });
    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus(
        response.status === 404
          ? "That Zerant ID or credential type could not be found."
          : "Verification request could not be created. Check the details and try again.",
      );
      return;
    }

    const created = (await response.json()) as VerificationRequestItem;
    setRequests((current) => [created, ...current]);
    setHolderId("");
    setPurpose("");
    setStatus("Request sent. The recipient has five minutes to approve or deny it.");
  }

  if (!backendAvailable) {
    return (
      <main id="main" className="verifier-page">
        <section className="verifier-hero">
          <p className="eyebrow">Verify with Zerant</p>
          <h1>Verification is temporarily unavailable.</h1>
        </section>
      </main>
    );
  }

  if (!authenticated) {
    return (
      <main id="main" className="verifier-page">
        <section className="verifier-hero">
          <p className="eyebrow">Verify with Zerant</p>
          <h1>Ask for proof, not a person&apos;s entire profile.</h1>
          <p>
            Request a trusted credential from an accepted issuer without collecting unrelated
            personal information or wallet history.
          </p>
          <Link href="/vault" className="button">
            Connect to Zerant <span aria-hidden="true">→</span>
          </Link>
        </section>
      </main>
    );
  }

  if (!profile) {
    return (
      <main id="main" className="verifier-page">
        <section className="verifier-hero">
          <p className="eyebrow">Verify with Zerant</p>
          <h1>Connect your application to private trust.</h1>
          <p>
            Register the application or organization that will ask users for proof. Requests are
            tied to this website so a proof cannot be reused somewhere else.
          </p>
        </section>
        <section className="verifier-panel verifier-register">
          <label htmlFor="verifier-name">Application or organization name</label>
          <input
            id="verifier-name"
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            placeholder="Your application name"
          />
          <label htmlFor="verifier-origin">Website</label>
          <input
            id="verifier-origin"
            value={origin}
            onChange={(event) => setOrigin(event.target.value)}
            placeholder="https://yourapp.com"
          />
          <Button disabled={!displayName.trim() || !origin.trim()} onClick={activate}>
            Activate verification
          </Button>
          {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
        </section>
      </main>
    );
  }

  return (
    <main id="main" className="verifier-page">
      <section className="verifier-hero verifier-hero-row">
        <div>
          <p className="eyebrow">Verifier workspace</p>
          <h1>{profile.display_name}</h1>
          <p>Request only the trusted fact your application actually needs.</p>
        </div>
        <span className="pill">{profile.origin}</span>
      </section>

      <section className="verifier-security-section">
        <article className="verifier-panel">
          <p className="eyebrow">Verification security</p>
          <h2>Keep request signing healthy.</h2>
          <p className="muted">
            Routine rotation changes the key used for new requests while already-sent requests keep
            their normal short expiry.
          </p>
          <div className="vault-actions wrap">
            <Button variant="secondary" onClick={() => rotateVerifierKey(false)}>
              Rotate security key
            </Button>
            <Button variant="secondary" onClick={() => rotateVerifierKey(true)}>
              Replace compromised key
            </Button>
          </div>
        </article>

        <article className="verifier-panel">
          <p className="eyebrow">Key history</p>
          <h2>{keys.length} key{keys.length === 1 ? "" : "s"}</h2>
          <div className="key-history-list">
            {keys.length ? (
              keys.map((key, index) => (
                <article className="key-history-card" key={key.valid_from + String(index)}>
                  <div>
                    <strong>{key.active ? "Current security key" : "Previous security key"}</strong>
                    <span
                      className={
                        key.compromised
                          ? "credential-status revoked"
                          : key.active
                            ? "credential-status active"
                            : "request-status"
                      }
                    >
                      {key.compromised ? "Compromised" : key.active ? "Active" : "Retired"}
                    </span>
                  </div>
                  <p className="small muted">
                    Active since {new Date(key.valid_from).toLocaleDateString()}
                    {key.retired_at
                      ? " · retired " + new Date(key.retired_at).toLocaleDateString()
                      : ""}
                  </p>
                </article>
              ))
            ) : (
              <p className="muted">Security-key history will appear here.</p>
            )}
          </div>
        </article>
      </section>

      <section className="verifier-grid">
        <article className="verifier-panel">
          <p className="eyebrow">New request</p>
          <h2>What do you need to verify?</h2>

          <label htmlFor="verify-holder">Recipient Zerant ID</label>
          <input
            id="verify-holder"
            value={holderId}
            onChange={(event) => setHolderId(event.target.value)}
            placeholder="zr_..."
          />

          <label htmlFor="verify-purpose">Why do you need this?</label>
          <textarea
            id="verify-purpose"
            value={purpose}
            onChange={(event) => setPurpose(event.target.value)}
            rows={4}
            placeholder="Explain the decision this proof will be used for."
          />

          <label htmlFor="verify-schema">Trusted credential type</label>
          <select
            id="verify-schema"
            value={schemaId}
            onChange={(event) => setSchemaId(event.target.value)}
          >
            {availableSchemas.length ? (
              availableSchemas.map((schema) => (
                <option value={schema.id} key={schema.id}>
                  {schema.display_name} · {schema.issuer_name}
                </option>
              ))
            ) : (
              <option value="">No trusted credential types available yet</option>
            )}
          </select>

          {selectedSchema ? (
            <div className="selected-schema-summary">
              <strong>{selectedSchema.display_name}</strong>
              <p>{selectedSchema.description}</p>
              <span className="small muted">
                Issued by {selectedSchema.issuer_name} · {selectedSchema.context}
              </span>
            </div>
          ) : null}

          <p className="small muted">
            Requests are short-lived. The recipient has five minutes to review and respond.
          </p>
          <Button
            disabled={!holderId.trim() || !purpose.trim() || !selectedSchema}
            onClick={createRequest}
          >
            Send verification request
          </Button>
          {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
        </article>

        <article className="verifier-panel">
          <p className="eyebrow">Requests</p>
          <h2>{requests.length} request{requests.length === 1 ? "" : "s"}</h2>
          <div className="verification-list">
            {requests.length ? (
              requests.map((request) => (
                <article className="verification-card" key={request.id}>
                  <div className="verification-card-top">
                    <strong>{request.credential_name ?? "Legacy credential"}</strong>
                    <span className={"request-status " + request.status}>
                      {request.verified ? "Verified" : request.status}
                    </span>
                  </div>
                  <p>{request.purpose}</p>
                  <p className="small muted">
                    {request.context} · {request.holder_zerant_id}
                  </p>
                </article>
              ))
            ) : (
              <p className="muted">No verification requests yet.</p>
            )}
          </div>
        </article>
      </section>
    </main>
  );
}
