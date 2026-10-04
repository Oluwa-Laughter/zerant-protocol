"use client";

import { useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

export type VerifierProfile = {
  display_name: string;
  origin: string;
  created_at: string;
};

export type TrustedIssuerOption = {
  display_name: string;
  issuer_id: string;
};

export type VerificationRequestItem = {
  id: string;
  holder_zerant_id: string;
  purpose: string;
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
}: {
  authenticated: boolean;
  backendAvailable: boolean;
  initialProfile: VerifierProfile | null;
  issuers: TrustedIssuerOption[];
  initialRequests: VerificationRequestItem[];
}) {
  const [profile, setProfile] = useState(initialProfile);
  const [requests, setRequests] = useState(initialRequests);
  const [displayName, setDisplayName] = useState("");
  const [origin, setOrigin] = useState("");
  const [holderId, setHolderId] = useState("");
  const [purpose, setPurpose] = useState("");
  const [claimType, setClaimType] = useState("");
  const [context, setContext] = useState("");
  const [issuerId, setIssuerId] = useState(issuers[0]?.issuer_id ?? "");
  const [status, setStatus] = useState("");

  async function activate() {
    const response = await fetch("/api/zerant/verifier", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ display_name: displayName, origin }),
    });
    if (!response.ok) {
      if (response.status === 429) { setStatus("You’re doing that too quickly. Try again in a minute."); return; }
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

  async function createRequest() {
    if (!issuerId) {
      setStatus("Choose a trusted issuer first.");
      return;
    }
    const response = await fetch("/api/zerant/verifier/requests", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        holder_zerant_id: holderId,
        purpose,
        claim_type: claimType,
        context,
        accepted_issuer_ids: [issuerId],
      }),
    });
    if (!response.ok) {
      if (response.status === 429) { setStatus("You’re doing that too quickly. Try again in a minute."); return; }
      setStatus(
        response.status === 404
          ? "That Zerant ID could not be found."
          : "Verification request could not be created. Check the details and try again.",
      );
      return;
    }
    const created = (await response.json()) as VerificationRequestItem;
    setRequests((current) => [created, ...current]);
    setHolderId("");
    setPurpose("");
    setClaimType("");
    setContext("");
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
            Create narrow verification requests for membership, roles, contributions,
            achievements or eligibility without collecting unrelated personal information.
          </p>
          <Link href="/vault" className="button">Connect to Zerant <span aria-hidden="true">→</span></Link>
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
          <p>Request only the trust signal your application actually needs.</p>
        </div>
        <span className="pill">{profile.origin}</span>
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

          <label htmlFor="verify-claim">What should they prove?</label>
          <input
            id="verify-claim"
            value={claimType}
            onChange={(event) => setClaimType(event.target.value)}
            placeholder="Membership, contributor, completion, role..."
          />

          <label htmlFor="verify-context">Where does this apply?</label>
          <input
            id="verify-context"
            value={context}
            onChange={(event) => setContext(event.target.value)}
            placeholder="Community, program, project or domain"
          />

          <label htmlFor="verify-issuer">Trusted issuer</label>
          <select
            id="verify-issuer"
            value={issuerId}
            onChange={(event) => setIssuerId(event.target.value)}
          >
            {issuers.length ? (
              issuers.map((issuer) => (
                <option value={issuer.issuer_id} key={issuer.issuer_id}>
                  {issuer.display_name}
                </option>
              ))
            ) : (
              <option value="">No issuers available yet</option>
            )}
          </select>

          <p className="small muted">
            Requests are short-lived. The recipient has five minutes to review and respond.
          </p>
          <Button
            disabled={
              !holderId.trim() ||
              !purpose.trim() ||
              !claimType.trim() ||
              !context.trim() ||
              !issuerId
            }
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
            {requests.length ? requests.map((request) => (
              <article className="verification-card" key={request.id}>
                <div className="verification-card-top">
                  <strong>{request.claim_type}</strong>
                  <span className={"request-status " + request.status}>
                    {request.verified ? "Verified" : request.status}
                  </span>
                </div>
                <p>{request.purpose}</p>
                <p className="small muted">{request.context} · {request.holder_zerant_id}</p>
              </article>
            )) : (
              <p className="muted">No verification requests yet.</p>
            )}
          </div>
        </article>
      </section>
    </main>
  );
}
