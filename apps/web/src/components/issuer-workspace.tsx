"use client";

import { useMemo, useRef, useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

export type IssuerProfile = {
  display_name: string;
  issuer_id: string;
  retired_at: string | null;
  created_at: string;
};

export type IssuerMember = {
  zerant_id: string;
  role: "owner" | "admin" | "issuer" | "auditor";
  owner: boolean;
  joined_at: string;
};

export type IssuerInvitation = {
  id: string;
  issuer_name: string;
  issuer_id: string;
  invited_zerant_id: string;
  role: "admin" | "issuer" | "auditor";
  status: string;
  created_at: string;
  expires_at: string;
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
  version: number;
  active: boolean;
  supersedes_schema_id: string | null;
  retired_at: string | null;
  created_at: string;
};



export type IssuerKeyView = {
  active: boolean;
  compromised: boolean;
  valid_from: string;
  retired_at: string | null;
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

export type IssuerActivityEvent = {
  id: number;
  event_type: string;
  actor_zerant_id: string;
  object_id: string;
  label: string;
  context: string | null;
  counterparty: string | null;
  created_at: string;
};

export type IssuerActivityPage = {
  items: IssuerActivityEvent[];
  next_cursor: string | null;
};

type CredentialIssueReview = {
  holderId: string;
  schemaId: string;
  schemaName: string;
  context: string;
  expiryDays: number;
  value: string;
};

function activityTitle(type: string): string {
  const labels: Record<string, string> = {
    team_invited: "Team invitation sent",
    team_joined: "Team member joined",
    team_declined: "Team invitation declined",
    team_member_removed: "Team member removed",
    ownership_transferred: "Ownership transferred",
    issuer_key_rotated: "Security key rotated",
    issuer_key_compromised: "Security key marked compromised",
    credential_schema_created: "Credential type created",
    credential_schema_versioned: "Credential type version published",
    credential_schema_retired: "Credential type retired",
    credential_issued: "Credential issued",
    credential_revoked: "Credential revoked",
    issuer_retired: "Issuer retired",
  };
  return labels[type] ?? "Organization activity";
}

export function IssuerWorkspace({
  authenticated,
  backendAvailable,
  currentZerantId,
  initialProfile,
  initialIssued,
  initialSchemas,
  initialKeys,
  initialTeam,
  initialTeamInvitations,
  initialMyInvitations,
  initialActivity,
}: {
  authenticated: boolean;
  backendAvailable: boolean;
  currentZerantId: string | null;
  initialProfile: IssuerProfile | null;
  initialIssued: IssuedCredential[];
  initialSchemas: CredentialSchema[];
  initialKeys: IssuerKeyView[];
  initialTeam: IssuerMember[];
  initialTeamInvitations: IssuerInvitation[];
  initialMyInvitations: IssuerInvitation[];
  initialActivity: IssuerActivityPage;
}) {
  const [profile, setProfile] = useState<IssuerProfile | null>(initialProfile);
  const [issued, setIssued] = useState<IssuedCredential[]>(initialIssued);
  const [schemas, setSchemas] = useState<CredentialSchema[]>(initialSchemas);
  const [keys, setKeys] = useState<IssuerKeyView[]>(initialKeys);
  const [team, setTeam] = useState<IssuerMember[]>(initialTeam);
  const [teamInvitations, setTeamInvitations] =
    useState<IssuerInvitation[]>(initialTeamInvitations);
  const [myInvitations, setMyInvitations] =
    useState<IssuerInvitation[]>(initialMyInvitations);
  const [activity, setActivity] = useState<IssuerActivityEvent[]>(
    initialActivity.items,
  );
  const [activityCursor, setActivityCursor] = useState<string | null>(
    initialActivity.next_cursor,
  );
  const [activityLoading, setActivityLoading] = useState(false);
  const [inviteZerantId, setInviteZerantId] = useState("");
  const [inviteRole, setInviteRole] =
    useState<"admin" | "issuer" | "auditor">("issuer");
  const [displayName, setDisplayName] = useState("");
  const [holderId, setHolderId] = useState("");
  const [issueReview, setIssueReview] = useState<CredentialIssueReview | null>(null);
  const [issuing, setIssuing] = useState(false);
  const issuingRef = useRef(false);
  const [schemaId, setSchemaId] = useState(initialSchemas.find((item) => item.active)?.id ?? "");
  const [value, setValue] = useState("");
  const [schemaName, setSchemaName] = useState("");
  const [schemaDescription, setSchemaDescription] = useState("");
  const [schemaContext, setSchemaContext] = useState("");
  const [schemaExpiry, setSchemaExpiry] = useState("90");
  const [editingSchemaId, setEditingSchemaId] = useState<string | null>(null);
  const [versionDescription, setVersionDescription] = useState("");
  const [versionExpiry, setVersionExpiry] = useState("90");
  const [status, setStatus] = useState("");
  const [retireConfirm, setRetireConfirm] = useState("");
  const [retiring, setRetiring] = useState(false);

  const activeSchemas = useMemo(
    () => schemas.filter((item) => item.active),
    [schemas],
  );

  const currentMember = useMemo(
    () => team.find((item) => item.zerant_id === currentZerantId) ?? null,
    [team, currentZerantId],
  );
  const currentRole = currentMember?.role ?? null;
  const issuerRetired = Boolean(profile?.retired_at);
  const canManageTeam = currentRole === "owner" || currentRole === "admin";
  const canInviteTeam = canManageTeam && !issuerRetired;
  const canInviteSuccessor = issuerRetired && currentRole === "owner";
  const canTransferOwnership = currentRole === "owner";
  const canManageSecurity = currentRole === "owner" || currentRole === "admin";
  const canManageSchemas = (currentRole === "owner" || currentRole === "admin") && !issuerRetired;
  const canIssue =
    !issuerRetired && (currentRole === "owner" || currentRole === "admin" || currentRole === "issuer");

  const revokedIssued = issued.filter((item) => item.revoked).length;
  const activeIssued = issued.length - revokedIssued;

  async function loadMoreActivity() {
    if (!activityCursor || activityLoading) return;
    setActivityLoading(true);
    try {
      const response = await fetch(
        "/api/zerant/issuer/activity?limit=20&cursor=" +
          encodeURIComponent(activityCursor),
        {
          credentials: "same-origin",
          cache: "no-store",
        },
      );
      if (!response.ok) {
        setStatus("Organization history could not be loaded.");
        return;
      }
      const page = (await response.json()) as IssuerActivityPage;
      setActivity((current) => [...current, ...page.items]);
      setActivityCursor(page.next_cursor);
    } finally {
      setActivityLoading(false);
    }
  }

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

  async function inviteTeamMember() {
    const response = await fetch("/api/zerant/issuer/team/invitations", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        zerant_id: inviteZerantId,
        role: canInviteSuccessor ? "admin" : inviteRole,
      }),
    });

    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      if (response.status === 403) {
        setStatus("Your role cannot manage this team.");
        return;
      }
      setStatus(
        response.status === 404
          ? "That Zerant ID could not be found."
          : response.status === 409
            ? "That person already belongs to an issuer organization."
            : "Invitation could not be sent.",
      );
      return;
    }

    const invitation = (await response.json()) as IssuerInvitation;
    setTeamInvitations((current) => [
      invitation,
      ...current.filter((item) => item.invited_zerant_id !== invitation.invited_zerant_id),
    ]);
    setInviteZerantId("");
    setStatus(canInviteSuccessor
      ? "Successor invitation sent. After they accept as admin, transfer ownership to them before deleting your personal account."
      : "Invitation sent. The recipient must accept it from their Zerant account.");
  }

  async function decideInvitation(id: string, decision: "accept" | "decline") {
    const response = await fetch(
      "/api/zerant/issuer/invitations/" + encodeURIComponent(id) + "/decision",
      {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ decision }),
      },
    );

    if (!response.ok) {
      setStatus(
        response.status === 409
          ? "This invitation is no longer available or you already belong to an issuer."
          : "Invitation could not be updated.",
      );
      return;
    }

    setMyInvitations((current) => current.filter((item) => item.id !== id));
    if (decision === "accept") {
      window.location.reload();
      return;
    }
    setStatus("Invitation declined.");
  }

  async function removeTeamMember(zerantId: string) {
    const response = await fetch(
      "/api/zerant/issuer/team/" + encodeURIComponent(zerantId),
      {
        method: "DELETE",
        credentials: "same-origin",
      },
    );

    if (!response.ok) {
      setStatus(
        response.status === 403
          ? "Your role cannot remove this team member."
          : "Team member could not be removed.",
      );
      return;
    }
    setTeam((current) => current.filter((item) => item.zerant_id !== zerantId));
    setStatus("Team member removed.");
  }

  async function transferOwnership(zerantId: string) {
    const response = await fetch("/api/zerant/issuer/ownership/transfer", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ zerant_id: zerantId }),
    });

    if (!response.ok) {
      setStatus(
        response.status === 403
          ? "Only the current owner can transfer ownership."
          : "Ownership could not be transferred.",
      );
      return;
    }

    setStatus("Ownership transferred securely.");
    window.location.reload();
  }

  async function retireIssuer() {
    if (retiring || retireConfirm !== "RETIRE") return;
    setRetiring(true);
    try {
      const response = await fetch("/api/zerant/issuer", {
        method: "DELETE",
        credentials: "same-origin",
      });
      if (!response.ok) {
        setStatus(
          response.status === 403
            ? "Sign in again recently and use the owner account before retiring this issuer."
            : response.status === 409
              ? "This issuer is already retired or its state changed. Refresh and try again."
              : "Issuer retirement could not be completed.",
        );
        return;
      }
      const now = new Date().toISOString();
      setProfile((current) => current ? { ...current, retired_at: now } : current);
      setSchemas((current) => current.map((schema) =>
        schema.active ? { ...schema, active: false, retired_at: schema.retired_at ?? now } : schema,
      ));
      setTeamInvitations([]);
      setRetireConfirm("");
      setStatus("Issuer retired. Historical credentials remain verifiable, and security maintenance remains available.");
    } finally {
      setRetiring(false);
    }
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

  function beginCredentialTypeVersion(schema: CredentialSchema) {
    setEditingSchemaId(schema.id);
    setVersionDescription(schema.description);
    setVersionExpiry(String(schema.default_expiry_days));
  }

  async function publishCredentialTypeVersion() {
    if (!editingSchemaId) return;
    const days = Number.parseInt(versionExpiry, 10);
    if (!Number.isInteger(days) || !versionDescription.trim()) {
      setStatus("Enter a description and valid lifetime for the new version.");
      return;
    }

    const response = await fetch(
      "/api/zerant/issuer/schemas/" + encodeURIComponent(editingSchemaId) + "/versions",
      {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          description: versionDescription,
          default_expiry_days: days,
        }),
      },
    );

    if (!response.ok) {
      if (response.status === 429) {
        setStatus("You’re doing that too quickly. Try again in a minute.");
        return;
      }
      setStatus(
        response.status === 409
          ? "This credential type changed while you were editing it. Refresh and try again."
          : "New credential version could not be published.",
      );
      return;
    }

    const created = (await response.json()) as CredentialSchema;
    const now = new Date().toISOString();
    setSchemas((current) => [
      created,
      ...current.map((item) =>
        item.id === editingSchemaId
          ? { ...item, active: false, retired_at: now }
          : item,
      ),
    ]);
    setSchemaId(created.id);
    setEditingSchemaId(null);
    setVersionDescription("");
    setVersionExpiry("90");
    setStatus(
      "New credential version published. Existing credentials keep their original definition.",
    );
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

  async function rotateIssuerKey(compromiseCurrent: boolean) {
    const response = await fetch("/api/zerant/issuer/keys/rotate", {
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
      setStatus("Security key could not be replaced.");
      return;
    }

    const created = (await response.json()) as IssuerKeyView;
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
      const issuedResponse = await fetch("/api/zerant/issuer/credentials", {
        credentials: "same-origin",
        cache: "no-store",
      });
      if (issuedResponse.ok) {
        setIssued((await issuedResponse.json()) as IssuedCredential[]);
      }
    }

    setStatus(
      compromiseCurrent
        ? "Compromised key replaced. Credentials signed by that key are no longer valid."
        : "Security key rotated. Existing credentials remain valid.",
    );
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

  function reviewCredentialIssue() {
    const recipient = holderId.trim();
    const claim = value.trim();
    const schema = activeSchemas.find((item) => item.id === schemaId);
    if (!/^zr_[0-9a-f]{24}$/.test(recipient)) {
      setStatus("Enter the recipient’s complete Zerant ID from their vault.");
      return;
    }
    if (!schema || !claim || claim.length > 512) {
      setStatus("Choose an active credential type and enter a claim of up to 512 characters.");
      return;
    }
    setIssueReview({
      holderId: recipient,
      schemaId: schema.id,
      schemaName: schema.display_name,
      context: schema.context,
      expiryDays: schema.default_expiry_days,
      value: claim,
    });
    setStatus("");
  }

  async function issueCredential() {
    if (!issueReview || !canIssue || issuingRef.current) return;
    issuingRef.current = true;
    setIssuing(true);

    try {
      const response = await fetch("/api/zerant/issuer/credentials", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          holder_zerant_id: issueReview.holderId,
          credential_schema_id: issueReview.schemaId,
          value: issueReview.value,
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
      setIssueReview(null);
      setStatus("Credential issued and delivered to the recipient.");
    } catch {
      setStatus("Could not confirm the result. Check issued credentials before trying again.");
    } finally {
      issuingRef.current = false;
      setIssuing(false);
    }
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
          <h1>Issue credentials as an organization.</h1>
          <p>
            Create a profile for your organization, community, team or project. An authorized
            team member can then send a credential to someone using their Zerant ID. No wallet
            address is needed.
          </p>
        </section>

        <section className="issuer-start-steps" aria-label="How organization issuance works">
          <h2>How issuing works</h2>
          <ol>
            <li><strong>Set up your organization.</strong> Name the group that stands behind each claim.</li>
            <li><strong>Define a credential type.</strong> Choose the fact you can substantiate, such as membership or completion.</li>
            <li><strong>Send it to a holder.</strong> Ask for their Zerant ID, check the evidence, and issue the credential to their private vault.</li>
          </ol>
          <p className="small muted">The holder later approves each verification request. Issuing a credential does not publish it or grant the organization access to a wallet.</p>
        </section>

        {myInvitations.length ? (
          <section className="issuer-team-invitations">
            <div className="section-heading">
              <p className="eyebrow">Organization invitations</p>
              <h2>You’ve been invited to help manage an issuer.</h2>
            </div>
            <div className="team-invitation-list">
              {myInvitations.map((invitation) => (
                <article className="team-invitation-card" key={invitation.id}>
                  <div>
                    <strong>{invitation.issuer_name}</strong>
                    <span className="team-role-badge">{invitation.role}</span>
                  </div>
                  <p className="small muted">
                    Invitation expires {new Date(invitation.expires_at).toLocaleDateString()}.
                  </p>
                  <div className="vault-actions wrap">
                    <Button onClick={() => decideInvitation(invitation.id, "accept")}>
                      Accept invitation
                    </Button>
                    <Button
                      variant="secondary"
                      onClick={() => decideInvitation(invitation.id, "decline")}
                    >
                      Decline
                    </Button>
                  </div>
                </article>
              ))}
            </div>
          </section>
        ) : null}

        <section className="issuer-panel issuer-create-panel">
          <p className="eyebrow">Create a new issuer</p>
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
          {myInvitations.length ? (
            <p className="small muted">
              Creating a new issuer means you will not be able to accept another issuer team
              invitation with this Zerant account.
            </p>
          ) : null}
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
        <span className={issuerRetired ? "pill issuer-retired-pill" : "pill"}>
          {issuerRetired ? "Retired" : currentRole ? currentRole.charAt(0).toUpperCase() + currentRole.slice(1) : "Issuer team"}
        </span>
      </section>

      {issuerRetired ? (
        <section className="issuer-retired-banner" aria-label="Retired issuer status">
          <div><p className="eyebrow">Retired issuer</p><h2>New trust creation is closed.</h2></div>
          <p>Existing credentials, revocation, security keys, organization history, team cleanup, and ownership transfer remain available. New credential types and credential issuance stay disabled. The owner may invite an admin successor solely to complete ownership transfer.</p>
        </section>
      ) : null}

      <section className="issuer-progress" aria-label="Issuer setup and issuance progress">
        <div className="issuer-progress-heading">
          <div><p className="eyebrow">Issuance flow</p><h2>From organization setup to a live credential.</h2></div>
          <span className="small muted">Use Zerant IDs · no wallet address required</span>
        </div>
        <div className="issuer-progress-grid">
          <article className="issuer-progress-step complete">
            <span className="issuer-progress-number">01</span>
            <div><strong>Organization</strong><span>{profile.display_name}</span></div>
            <span className="issuer-progress-state">Complete</span>
          </article>
          <a className={activeSchemas.length ? "issuer-progress-step complete" : "issuer-progress-step current"} href="#credential-types">
            <span className="issuer-progress-number">02</span>
            <div><strong>Credential type</strong><span>{activeSchemas.length ? `${activeSchemas.length} active type${activeSchemas.length === 1 ? "" : "s"}` : "Define what your organization can prove"}</span></div>
            <span className="issuer-progress-state">{activeSchemas.length ? "Ready" : "Next"}</span>
          </a>
          <a className={issued.length ? "issuer-progress-step complete" : activeSchemas.length ? "issuer-progress-step current" : "issuer-progress-step"} href="#issue-credential">
            <span className="issuer-progress-number">03</span>
            <div><strong>Issue privately</strong><span>{issued.length ? `${issued.length} credential${issued.length === 1 ? "" : "s"} delivered` : "Send to a holder Zerant ID"}</span></div>
            <span className="issuer-progress-state">{issued.length ? "Active" : activeSchemas.length ? "Next" : "Waiting"}</span>
          </a>
          <article className={issued.length ? "issuer-progress-step complete" : "issuer-progress-step"}>
            <span className="issuer-progress-number">04</span>
            <div><strong>Manage lifecycle</strong><span>{issued.length ? `${activeIssued} active · ${revokedIssued} revoked` : "Revocation and history begin after issuance"}</span></div>
            <span className="issuer-progress-state">{issued.length ? "Ready" : "Waiting"}</span>
          </article>
        </div>
      </section>

      <section className="issuer-team-section">
        <div className="section-heading">
          <p className="eyebrow">Organization team</p>
          <h2>Separate responsibilities without sharing accounts.</h2>
          <p className="muted">
            Owners control ownership, admins manage configuration and are the only eligible ownership successors, issuers can issue and revoke, and auditors have read-only access.
          </p>
        </div>

        <div className="issuer-team-grid">
          <article className="issuer-panel">
            <p className="eyebrow">Members</p>
            <h3>{team.length} team member{team.length === 1 ? "" : "s"}</h3>
            <div className="issuer-member-list">
              {team.map((member) => {
                const canRemove =
                  canManageTeam &&
                  !member.owner &&
                  member.zerant_id !== currentZerantId &&
                  !(currentRole === "admin" && member.role === "admin");
                return (
                  <article className="issuer-member-card" key={member.zerant_id}>
                    <div>
                      <span className="mono small">{member.zerant_id}</span>
                      <span className="team-role-badge">{member.role}</span>
                    </div>
                    <p className="small muted">
                      {member.owner
                        ? "Controls organization ownership and recovery-sensitive changes."
                        : "Joined " + new Date(member.joined_at).toLocaleDateString()}
                    </p>
                    <div className="vault-actions wrap">
                      {canRemove ? (
                        <Button
                          variant="secondary"
                          onClick={() => removeTeamMember(member.zerant_id)}
                        >
                          Remove
                        </Button>
                      ) : null}
                      {canTransferOwnership && !member.owner && member.role === "admin" ? (
                        <Button
                          variant="secondary"
                          onClick={() => transferOwnership(member.zerant_id)}
                        >
                          Transfer ownership
                        </Button>
                      ) : null}
                    </div>
                  </article>
                );
              })}
            </div>
          </article>

          {canInviteTeam || canInviteSuccessor ? (
            <article className={canInviteSuccessor ? "issuer-panel issuer-successor-panel" : "issuer-panel"}>
              <p className="eyebrow">{canInviteSuccessor ? "Ownership succession" : "Invite teammate"}</p>
              <h3>{canInviteSuccessor ? "Add an admin successor before transferring ownership." : "Add responsibility by Zerant ID."}</h3>
              {canInviteSuccessor ? <p className="small muted">This retired issuer remains archive-only. The invited account can join only as an admin so you can transfer ownership; retirement does not reopen issuance or configuration.</p> : null}
              <label htmlFor="team-zerant-id">Teammate Zerant ID</label>
              <input
                id="team-zerant-id"
                value={inviteZerantId}
                onChange={(event) => setInviteZerantId(event.target.value)}
                placeholder="zr_..."
              />
              <label htmlFor="team-role">Role</label>
              {canInviteSuccessor ? (
                <div id="team-role" className="issuer-successor-role"><strong>Admin successor</strong><span>Required for retired-issuer ownership transfer.</span></div>
              ) : (
                <select
                  id="team-role"
                  value={inviteRole}
                  onChange={(event) =>
                    setInviteRole(event.target.value as "admin" | "issuer" | "auditor")
                  }
                >
                  <option value="issuer">Issuer — issue and revoke credentials</option>
                  <option value="admin">Admin — manage issuer configuration and team</option>
                  <option value="auditor">Auditor — read-only access</option>
                </select>
              )}
              <Button
                disabled={!inviteZerantId.trim()}
                onClick={inviteTeamMember}
              >
                {canInviteSuccessor ? "Invite successor" : "Send invitation"}
              </Button>

              {teamInvitations.length ? (
                <div className="pending-team-invitations">
                  <span className="eyebrow">Pending</span>
                  {teamInvitations.map((invitation) => (
                    <div className="pending-team-invitation" key={invitation.id}>
                      <span className="mono small">{invitation.invited_zerant_id}</span>
                      <span className="team-role-badge">{invitation.role}</span>
                    </div>
                  ))}
                </div>
              ) : null}
            </article>
          ) : (
            <article className="issuer-panel">
              <p className="eyebrow">Your access</p>
              <h3>{currentRole === "auditor" ? "Read-only access" : "Credential operations"}</h3>
              <p className="muted">
                {currentRole === "auditor"
                  ? "You can review issuer state and history but cannot change credentials or security settings."
                  : "You can issue and revoke credentials. Team and security configuration remain with owners and admins."}
              </p>
            </article>
          )}
        </div>
      </section>

      <section className="issuer-security-section">
        <article className="issuer-panel">
          <p className="eyebrow">Issuer security</p>
          <h2>Keep your issuing authority healthy.</h2>
          <p className="muted">
            Routine rotation changes the key used for new credentials while preserving credentials
            you already issued.
          </p>
          {canManageSecurity ? (
            <div className="vault-actions wrap">
              <Button variant="secondary" onClick={() => rotateIssuerKey(false)}>
                Rotate security key
              </Button>
              <Button variant="secondary" onClick={() => rotateIssuerKey(true)}>
                Replace compromised key
              </Button>
            </div>
          ) : (
            <p className="small muted">
              Security changes are limited to organization owners and admins.
            </p>
          )}
        </article>

        <article className="issuer-panel">
          <p className="eyebrow">Key history</p>
          <h2>{keys.length} key{keys.length === 1 ? "" : "s"}</h2>
          <div className="key-history-list">
            {keys.length ? (
              keys.map((key, index) => (
                <article className="key-history-card" key={key.valid_from + String(index)}>
                  <div>
                    <strong>
                      {key.active ? "Current security key" : "Previous security key"}
                    </strong>
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

      <section className="issuer-schema-section" id="credential-types">
        <article className="issuer-panel">
          <p className="eyebrow">Credential types</p>
          <h2>Define trust once. Reuse it consistently.</h2>

          <label htmlFor="schema-name">Credential name</label>
          <input
            id="schema-name"
            value={schemaName}
            onChange={(event) => setSchemaName(event.target.value)}
            placeholder="Membership, Program Completion, Contributor..."
            disabled={!canManageSchemas}
          />

          <label htmlFor="schema-description">What does it prove?</label>
          <textarea
            id="schema-description"
            value={schemaDescription}
            onChange={(event) => setSchemaDescription(event.target.value)}
            rows={3}
            placeholder="Describe what a holder is entitled to prove with this credential."
            disabled={!canManageSchemas}
          />

          <label htmlFor="schema-context">Where does it apply?</label>
          <input
            id="schema-context"
            value={schemaContext}
            onChange={(event) => setSchemaContext(event.target.value)}
            placeholder="Community, program, marketplace or organization"
            disabled={!canManageSchemas}
          />

          <label htmlFor="schema-expiry">Default validity</label>
          <select
            id="schema-expiry"
            value={schemaExpiry}
            onChange={(event) => setSchemaExpiry(event.target.value)}
            disabled={!canManageSchemas}
          >
            <option value="30">30 days</option>
            <option value="90">90 days</option>
            <option value="180">180 days</option>
            <option value="365">1 year</option>
          </select>

          <Button
            disabled={
              !canManageSchemas ||
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
                    <div>
                      <strong>{schema.display_name}</strong>
                      <span className="schema-version">v{schema.version}</span>
                    </div>
                    <span className={schema.active ? "credential-status active" : "credential-status revoked"}>
                      {schema.active ? "Active" : "Retired"}
                    </span>
                  </div>
                  <p>{schema.description}</p>
                  <p className="small muted">
                    {schema.context} · {schema.default_expiry_days} day validity
                    {schema.retired_at
                      ? " · retired " + new Date(schema.retired_at).toLocaleDateString()
                      : ""}
                  </p>
                  {schema.active && canManageSchemas ? (
                    <div className="schema-actions">
                      <Button
                        variant="secondary"
                        onClick={() => beginCredentialTypeVersion(schema)}
                      >
                        Publish new version
                      </Button>
                      <Button
                        variant="secondary"
                        onClick={() => deactivateCredentialType(schema.id)}
                      >
                        Retire credential type
                      </Button>
                    </div>
                  ) : null}
                  {editingSchemaId === schema.id ? (
                    <div className="schema-version-editor">
                      <label htmlFor={"version-description-" + schema.id}>Updated description</label>
                      <textarea
                        id={"version-description-" + schema.id}
                        value={versionDescription}
                        onChange={(event) => setVersionDescription(event.target.value)}
                        rows={3}
                      />
                      <label htmlFor={"version-expiry-" + schema.id}>Default validity</label>
                      <select
                        id={"version-expiry-" + schema.id}
                        value={versionExpiry}
                        onChange={(event) => setVersionExpiry(event.target.value)}
                      >
                        <option value="30">30 days</option>
                        <option value="90">90 days</option>
                        <option value="180">180 days</option>
                        <option value="365">1 year</option>
                      </select>
                      <div className="vault-actions wrap">
                        <Button onClick={publishCredentialTypeVersion}>Publish version</Button>
                        <Button
                          variant="secondary"
                          onClick={() => setEditingSchemaId(null)}
                        >
                          Cancel
                        </Button>
                      </div>
                    </div>
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

      {currentRole === "owner" ? (
        <section className="issuer-retirement-section">
          <div className="section-heading">
            <p className="eyebrow">Organization lifecycle</p>
            <h2>{issuerRetired ? "This issuer is retired." : "Retire this issuer safely."}</h2>
            <p className="muted">
              {issuerRetired
                ? "Historical credentials and verification material are preserved. Transfer ownership before deleting the owner’s personal Zerant account."
                : "Retirement stops new credentials, credential types, and invitations. Existing credentials remain verifiable and can still be revoked if necessary."}
            </p>
          </div>
          {!issuerRetired ? (
            <div className="issuer-retirement-control">
              <label htmlFor="retire-issuer-confirm">Type RETIRE to confirm</label>
              <input
                id="retire-issuer-confirm"
                value={retireConfirm}
                onChange={(event) => setRetireConfirm(event.target.value)}
                autoComplete="off"
              />
              <Button variant="secondary" disabled={retiring || retireConfirm !== "RETIRE"} onClick={() => void retireIssuer()}>
                {retiring ? "Retiring…" : "Retire issuer"}
              </Button>
            </div>
          ) : null}
        </section>
      ) : null}

      <section className="issuer-activity-section">
        <div className="section-heading">
          <p className="eyebrow">Organization history</p>
          <h2>See the important changes made by your team.</h2>
          <p className="muted">
            Sensitive issuer actions are recorded in an append-only history so owners, admins and
            auditors can review who changed what and when.
          </p>
        </div>

        <div className="issuer-activity-list">
          {activity.length ? (
            activity.map((event) => (
              <article className="issuer-activity-card" key={event.id}>
                <div className="issuer-activity-card-top">
                  <div>
                    <strong>{activityTitle(event.event_type)}</strong>
                    <p className="small muted">{event.label}</p>
                  </div>
                  <time className="small muted" dateTime={event.created_at}>
                    {new Date(event.created_at).toLocaleString()}
                  </time>
                </div>
                <div className="issuer-activity-meta">
                  <span>
                    By <span className="mono">{event.actor_zerant_id}</span>
                  </span>
                  {event.context ? <span>{event.context}</span> : null}
                  {event.counterparty ? (
                    <span>
                      Related <span className="mono">{event.counterparty}</span>
                    </span>
                  ) : null}
                </div>
              </article>
            ))
          ) : (
            <div className="issuer-activity-empty">
              <p className="muted">
                New issuer actions will appear here as your organization operates.
              </p>
            </div>
          )}
        </div>

        {activityCursor ? (
          <Button
            variant="secondary"
            disabled={activityLoading}
            onClick={loadMoreActivity}
          >
            {activityLoading ? "Loading…" : "Load older activity"}
          </Button>
        ) : null}
      </section>

      <section className="issuer-grid" id="issue-credential">
        <article className="issuer-panel">
          <p className="eyebrow">Issue credential</p>
          <h2>Send a trusted credential.</h2>
          <p className="small muted">First confirm the recipient earned the claim. Ask them to copy their Zerant ID from their private credential vault. A Zcash payment address cannot receive a credential.</p>

          <label htmlFor="holder-id">Recipient Zerant ID</label>
          <input
            id="holder-id"
            value={holderId}
            onChange={(event) => setHolderId(event.target.value)}
            placeholder="zr_..."
            disabled={!canIssue || Boolean(issueReview)}
            aria-describedby="holder-id-help"
          />
          <p id="holder-id-help" className="small muted">This delivers to a Zerant account. It does not connect or identify a Zcash wallet.</p>

          <label htmlFor="credential-type">Credential type</label>
          <select
            id="credential-type"
            value={schemaId}
            onChange={(event) => setSchemaId(event.target.value)}
            disabled={!canIssue || Boolean(issueReview)}
          >
            {activeSchemas.length ? (
              activeSchemas.map((schema) => (
                <option value={schema.id} key={schema.id}>
                  {schema.display_name} v{schema.version} · {schema.context}
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
            disabled={!canIssue || Boolean(issueReview)}
          />

          {issueReview ? (
            <div className="credential-issue-review" aria-label="Review credential before issuing">
              <h3>Review before sending</h3>
              <dl>
                <div><dt>Recipient Zerant ID</dt><dd className="mono">{issueReview.holderId}</dd></div>
                <div><dt>Credential</dt><dd>{issueReview.schemaName}</dd></div>
                <div><dt>Claim</dt><dd>{issueReview.value}</dd></div>
                <div><dt>Context</dt><dd>{issueReview.context}</dd></div>
                <div><dt>Valid for</dt><dd>{issueReview.expiryDays} days from issuance</dd></div>
              </dl>
              <p className="small muted">Check the person and claim against your own records. Sending adds this credential to that Zerant account.</p>
              <div className="vault-actions wrap">
                <Button disabled={issuing} onClick={issueCredential}>{issuing ? "Issuing…" : "Confirm and issue"}</Button>
                <Button variant="secondary" disabled={issuing} onClick={() => setIssueReview(null)}>Edit details</Button>
              </div>
            </div>
          ) : (
            <Button
              disabled={!canIssue || !holderId.trim() || !schemaId || !value.trim()}
              onClick={reviewCredentialIssue}
            >
              Review credential
            </Button>
          )}
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
                      {!item.revoked && canIssue ? (
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
