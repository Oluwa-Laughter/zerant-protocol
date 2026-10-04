"use client";

import { useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

export type IssuerProfile = {
  display_name: string;
  issuer_id: string;
  created_at: string;
};

export type IssuedCredential = {
  credential_id: string;
  holder_zerant_id: string;
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
}: {
  authenticated: boolean;
  backendAvailable: boolean;
  initialProfile: IssuerProfile | null;
  initialIssued: IssuedCredential[];
}) {
  const [profile, setProfile] = useState<IssuerProfile | null>(initialProfile);
  const [issued, setIssued] = useState<IssuedCredential[]>(initialIssued);
  const [displayName, setDisplayName] = useState("");
  const [holderId, setHolderId] = useState("");
  const [claimType, setClaimType] = useState("");
  const [value, setValue] = useState("");
  const [context, setContext] = useState("");
  const [expiresInDays, setExpiresInDays] = useState("90");
  const [status, setStatus] = useState("");

  async function activateIssuer() {
    const response = await fetch("/api/zerant/issuer", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ display_name: displayName }),
    });
    if (!response.ok) {
      setStatus(response.status === 409 ? "This account already has an issuer profile." : "Issuer profile could not be created.");
      return;
    }
    setProfile((await response.json()) as IssuerProfile);
    setDisplayName("");
    setStatus("Issuer profile is active.");
  }

  async function issueCredential() {
    const days = Number.parseInt(expiresInDays, 10);
    if (!Number.isInteger(days)) {
      setStatus("Choose a valid expiry period.");
      return;
    }

    const response = await fetch("/api/zerant/issuer/credentials", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        holder_zerant_id: holderId,
        claim_type: claimType,
        value,
        context,
        expires_in_days: days,
      }),
    });

    if (!response.ok) {
      setStatus(
        response.status === 404
          ? "That Zerant ID could not be found."
          : "Credential could not be issued. Check the details and try again.",
      );
      return;
    }

    const created = (await response.json()) as IssuedCredential;
    setIssued((current) => [created, ...current]);
    setHolderId("");
    setClaimType("");
    setValue("");
    setContext("");
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
          <Link href="/vault" className="button">Connect to Zerant <span aria-hidden="true">→</span></Link>
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
            active, you can send private credentials directly to a recipient&apos;s Zerant ID.
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
            Send private credentials to people who should be able to prove something about their
            relationship with your organization.
          </p>
        </div>
        <span className="pill">Issuer active</span>
      </section>

      <section className="issuer-grid">
        <article className="issuer-panel">
          <p className="eyebrow">Issue credential</p>
          <h2>What should this person be able to prove?</h2>

          <label htmlFor="holder-id">Recipient Zerant ID</label>
          <input
            id="holder-id"
            value={holderId}
            onChange={(event) => setHolderId(event.target.value)}
            placeholder="zr_..."
          />

          <label htmlFor="claim-type">Credential type</label>
          <input
            id="claim-type"
            value={claimType}
            onChange={(event) => setClaimType(event.target.value)}
            placeholder="Membership, contributor, completion, role..."
          />

          <label htmlFor="claim-value">Credential value</label>
          <input
            id="claim-value"
            value={value}
            onChange={(event) => setValue(event.target.value)}
            placeholder="What are you attesting to?"
          />

          <label htmlFor="credential-context">Context</label>
          <input
            id="credential-context"
            value={context}
            onChange={(event) => setContext(event.target.value)}
            placeholder="Community, program, project or domain"
          />

          <label htmlFor="credential-expiry">Valid for</label>
          <select
            id="credential-expiry"
            value={expiresInDays}
            onChange={(event) => setExpiresInDays(event.target.value)}
          >
            <option value="30">30 days</option>
            <option value="90">90 days</option>
            <option value="180">180 days</option>
            <option value="365">1 year</option>
          </select>

          <Button
            disabled={!holderId.trim() || !claimType.trim() || !value.trim() || !context.trim()}
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
              issued.map((item) => (
                <article className="issued-card" key={item.credential_id}>
                  <div>
                    <strong>{item.claim_type}</strong>
                    <span className="pill">{item.context}</span>
                  </div>
                  <p className="mono small">{item.holder_zerant_id}</p>
                  <p className="small muted">
                    Issued {new Date(item.issued_at).toLocaleDateString()} · valid until{" "}
                    {new Date(item.expires_at).toLocaleDateString()}
                  </p>
                </article>
              ))
            ) : (
              <p className="muted">No credentials have been issued from this profile yet.</p>
            )}
          </div>
        </article>
      </section>
    </main>
  );
}
