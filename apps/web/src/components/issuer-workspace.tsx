"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

export type IssuerProfile = {
  display_name: string;
  issuer_id: string;
  created_at: string;
};

export type CredentialSchema = {
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

export type IssuedCredential = {
  credential_id: string;
  holder_zerant_id: string;
  credential_schema_id: string | null;
  claim_type: string;
  context: string;
  issued_at: string;
  expires_at: string;
  revoked: boolean;
};

export function IssuerWorkspace({
  authenticated,
  backendAvailable,
  initialProfile,
  initialIssued,
  initialSchemas,
}: {
  authenticated: boolean;
  backendAvailable: boolean;
  initialProfile: IssuerProfile | null;
  initialIssued: IssuedCredential[];
  initialSchemas: CredentialSchema[];
}) {
  const [profile, setProfile] = useState<IssuerProfile | null>(initialProfile);
  const [issued, setIssued] = useState<IssuedCredential[]>(initialIssued);
  const [schemas, setSchemas] = useState<CredentialSchema[]>(initialSchemas);
  const [displayName, setDisplayName] = useState("");
  const [holderId, setHolderId] = useState("");
  const [schemaId, setSchemaId] = useState(initialSchemas.find((item) => item.active)?.id ?? "");
  const [value, setValue] = useState("");
  const [schemaName, setSchemaName] = useState("");
  const [schemaDescription, setSchemaDescription] = useState("");
  const [schemaContext, setSchemaContext] = useState("");
  const [schemaExpiry, setSchemaExpiry] = useState("90");
  const [status, setStatus] = useState("");

  const activeSchemas = useMemo(
    () => schemas.filter((item) => item.active),
    [schemas],
  );

  async function activateIssuer() {
    const response = await fetch("/api/zerant/issuer", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ display_name: displayName }),
    });
    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus(
        response.status === 409
          ? "This account already has an issuer profile."
          : "Issuer profile could not be created.",
      );
      return;
    }
    setProfile((await response.json()) as IssuerProfile);
    setDisplayName("");
    setStatus("Issuer profile is active.");
  }

  async function createCredentialType() {
    const days = Number.parseInt(schemaExpiry, 10);
    if (!Number.isInteger(days)) {
      setStatus("Choose a valid credential lifetime.");
      return;
    }

    const response = await fetch("/api/zerant/issuer/schemas", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        display_name: schemaName,
        description: schemaDescription,
        context: schemaContext,
        default_expiry_days: days,
      }),
    });

    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus(
        response.status === 409
          ? "A credential type with the same name or meaning already exists."
          : "Credential type could not be created.",
      );
      return;
    }

    const created = (await response.json()) as CredentialSchema;
    setSchemas((current) => [created, ...current]);
    setSchemaId(created.id);
    setSchemaName("");
    setSchemaDescription("");
    setSchemaContext("");
    setSchemaExpiry("90");
    setStatus("Credential type created. You can issue it immediately.");
  }

  async function deactivateCredentialType(schemaIdToDeactivate: string) {
    const response = await fetch(
      "/api/zerant/issuer/schemas/" + encodeURIComponent(schemaIdToDeactivate) + "/deactivate",
      { method: "POST", credentials: "same-origin" },
    );
    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus("Credential type could not be retired.");
      return;
    }

    const updated = (await response.json()) as CredentialSchema;
    setSchemas((current) =>
      current.map((item) => (item.id === updated.id ? updated : item)),
    );
    if (schemaId === updated.id) {
      const next = schemas.find((item) => item.id !== updated.id && item.active);
      setSchemaId(next?.id ?? "");
    }
    setStatus("Credential type retired. Existing credentials remain visible, but no new credentials or requests can use it.");
  }

  async function revokeCredential(credentialId: string) {
    const response = await fetch(
      "/api/zerant/issuer/credentials/" + encodeURIComponent(credentialId) + "/revoke",
      { method: "POST", credentials: "same-origin" },
    );
    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus("Credential could not be revoked. It may already be revoked.");
      return;
    }
    const updated = (await response.json()) as IssuedCredential;
    setIssued((current) =>
      current.map((item) =>
        item.credential_id === updated.credential_id ? updated : item,
      ),
    );
    setStatus("Credential revoked. It can no longer be used for new proofs.");
  }

  async function issueCredential() {
    if (!schemaId) {
      setStatus("Create or choose a credential type first.");
      return;
    }

    const response = await fetch("/api/zerant/issuer/credentials", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        holder_zerant_id: holderId,
        credential_schema_id: schemaId,
        value,
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
          : "Credential could not be issued. Check the details and try again.",
      );
      return;
    }

    const created = (await response.json()) as IssuedCredential;
    setIssued((current) => [created, ...current]);
    setHolderId("");
    setValue("");
    setStatus("Credential issued and delivered to the recipient.");
  }

  if (!backendAvailable) {
    return (
      <main id="main" className="issuer-page">
        <section className="issuer-hero">
          <p className="eyebrow">For issuers</p>
          <h1>Trusted credential issuance is temporarily unavailable.</h1>
          <p>Zerant will not fall back to an insecure or local-only flow.</p>
        </section>
      </main>
    );
  }

  if (!authenticated) {
    return (
      <main id="main" className="issuer-page">
        <section className="issuer-hero">
          <p className="eyebrow">For issuers</p>
          <h1>Issue private, portable credentials.</h1>
          <p>
            Organizations can attest to membership, contributions, roles, achievements and
            eligibility without forcing recipients to publish their identity or wallet history.
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
      <main id="main" className="issuer-page">
        <section className="issuer-hero">
          <p className="eyebrow">For issuers</p>
          <h1>Become a trusted issuer on Zerant.</h1>
          <p>
            Create an issuer profile for your organization, community, team or project. Once
            active, you can define trusted credential types and issue them directly to Zerant IDs.
          </p>
        </section>

        <section className="issuer-panel">
          <label htmlFor="issuer-name">Organization or issuer name</label>
          <input
            id="issuer-name"
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            placeholder="Your organization name"
          />
          <Button disabled={displayName.trim().length < 2} onClick={activateIssuer}>
            Activate issuer profile
          </Button>
          {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
        </section>
      </main>
    );
  }

  return (
    <main id="main" className="issuer-page">
      <section className="issuer-hero issuer-hero-row">
        <div>
          <p className="eyebrow">Issuer workspace</p>
          <h1>{profile.display_name}</h1>
          <p>
            Define what your organization can attest to, then issue those credentials privately
            to people who need to prove them.
          </p>
        </div>
        <span className="pill">Issuer active</span>
      </section>

      <section className="issuer-schema-section">
        <article className="issuer-panel">
          <p className="eyebrow">Credential types</p>
          <h2>Define trust once. Reuse it consistently.</h2>

          <label htmlFor="schema-name">Credential name</label>
          <input
            id="schema-name"
            value={schemaName}
            onChange={(event) => setSchemaName(event.target.value)}
            placeholder="Membership, Program Completion, Contributor..."
          />

          <label htmlFor="schema-description">What does it prove?</label>
          <textarea
            id="schema-description"
            value={schemaDescription}
            onChange={(event) => setSchemaDescription(event.target.value)}
            rows={3}
            placeholder="Describe what a holder is entitled to prove with this credential."
          />

          <label htmlFor="schema-context">Where does it apply?</label>
          <input
            id="schema-context"
            value={schemaContext}
            onChange={(event) => setSchemaContext(event.target.value)}
            placeholder="Community, program, marketplace or organization"
          />

          <label htmlFor="schema-expiry">Default validity</label>
          <select
            id="schema-expiry"
            value={schemaExpiry}
            onChange={(event) => setSchemaExpiry(event.target.value)}
          >
            <option value="30">30 days</option>
            <option value="90">90 days</option>
            <option value="180">180 days</option>
            <option value="365">1 year</option>
          </select>

          <Button
            disabled={
              !schemaName.trim() ||
              !schemaDescription.trim() ||
              !schemaContext.trim()
            }
            onClick={createCredentialType}
          >
            Create credential type
          </Button>
        </article>

        <article className="issuer-panel">
          <p className="eyebrow">Your credential types</p>
          <h2>{schemas.length} defined</h2>
          <div className="schema-list">
            {schemas.length ? (
              schemas.map((schema) => (
                <article className="schema-card" key={schema.id}>
                  <div className="schema-card-top">
                    <strong>{schema.display_name}</strong>
                    <span className={schema.active ? "credential-status active" : "credential-status revoked"}>
                      {schema.active ? "Active" : "Inactive"}
                    </span>
                  </div>
                  <p>{schema.description}</p>
                  <p className="small muted">
                    {schema.context} · {schema.default_expiry_days} day validity
                  </p>
                  {schema.active ? (
                    <Button
                      variant="secondary"
                      onClick={() => deactivateCredentialType(schema.id)}
                    >
                      Retire credential type
                    </Button>
                  ) : null}
                </article>
              ))
            ) : (
              <p className="muted">
                Create your first credential type before issuing credentials.
              </p>
            )}
          </div>
        </article>
      </section>

      <section className="issuer-grid">
        <article className="issuer-panel">
          <p className="eyebrow">Issue credential</p>
          <h2>Send a trusted credential.</h2>

          <label htmlFor="holder-id">Recipient Zerant ID</label>
          <input
            id="holder-id"
            value={holderId}
            onChange={(event) => setHolderId(event.target.value)}
            placeholder="zr_..."
          />

          <label htmlFor="credential-type">Credential type</label>
          <select
            id="credential-type"
            value={schemaId}
            onChange={(event) => setSchemaId(event.target.value)}
          >
            {activeSchemas.length ? (
              activeSchemas.map((schema) => (
                <option value={schema.id} key={schema.id}>
                  {schema.display_name} · {schema.context}
                </option>
              ))
            ) : (
              <option value="">Create a credential type first</option>
            )}
          </select>

          <label htmlFor="claim-value">What are you attesting to?</label>
          <input
            id="claim-value"
            value={value}
            onChange={(event) => setValue(event.target.value)}
            placeholder="Active member, completed, maintainer..."
          />

          <Button
            disabled={!holderId.trim() || !schemaId || !value.trim()}
            onClick={issueCredential}
          >
            Issue credential
          </Button>
          {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
        </article>

        <article className="issuer-panel">
          <p className="eyebrow">Issued credentials</p>
          <h2>{issued.length} credential{issued.length === 1 ? "" : "s"} issued</h2>
          <div className="issued-list">
            {issued.length ? (
              issued.map((item) => {
                const schema = schemas.find((entry) => entry.id === item.credential_schema_id);
                return (
                  <article className="issued-card" key={item.credential_id}>
                    <div>
                      <strong>{schema?.display_name ?? item.claim_type}</strong>
                      <span className="pill">{item.context}</span>
                    </div>
                    <p className="mono small">{item.holder_zerant_id}</p>
                    <p className="small muted">
                      Issued {new Date(item.issued_at).toLocaleDateString()} · valid until{" "}
                      {new Date(item.expires_at).toLocaleDateString()}
                    </p>
                    <div className="issued-card-actions">
                      <span className={item.revoked ? "credential-status revoked" : "credential-status active"}>
                        {item.revoked ? "Revoked" : "Active"}
                      </span>
                      {!item.revoked ? (
                        <Button
                          variant="secondary"
                          onClick={() => revokeCredential(item.credential_id)}
                        >
                          Revoke
                        </Button>
                      ) : null}
                    </div>
                  </article>
                );
              })
            ) : (
              <p className="muted">No credentials have been issued from this profile yet.</p>
            )}
          </div>
        </article>
      </section>
    </main>
  );
}
