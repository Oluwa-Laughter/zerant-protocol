"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { PasskeyManager, type PasskeyView } from "@/components/passkey-manager";
import { SessionManager, type AccountSessionView } from "@/components/session-manager";
import { ZcashSignInManager, type LinkedZcashMethod } from "@/components/zcash-sign-in-manager";
import { WorkspaceSectionNav } from "@/components/workspace-section-nav";

export type AccountSummary = {
  zerant_id: string;
  credential_count: number;
  passkey_count: number;
  issuer_profile: string | null;
  issuer_role: string | null;
  issuer_retired: boolean;
  verifier_profile: string | null;
  verifier_retired: boolean;
  can_delete: boolean;
};

export type AccountSecuritySummary = {
  accessMethods: number;
  passkeys: number;
  zcashMethods: number;
  activeSessions: number;
  hasRedundantAccess: boolean;
};

export function accountSecuritySummary(
  passkeys: PasskeyView[],
  zcashMethods: LinkedZcashMethod[],
  sessions: AccountSessionView[],
): AccountSecuritySummary {
  const accessMethods = passkeys.length + zcashMethods.length;
  return {
    accessMethods,
    passkeys: passkeys.length,
    zcashMethods: zcashMethods.length,
    activeSessions: sessions.length,
    hasRedundantAccess: accessMethods >= 2,
  };
}

export function accountSecuritySummaryFromCounts(
  passkeys: number,
  zcashMethods: number,
  activeSessions: number,
): AccountSecuritySummary {
  const accessMethods = passkeys + zcashMethods;
  return {
    accessMethods,
    passkeys,
    zcashMethods,
    activeSessions,
    hasRedundantAccess: accessMethods >= 2,
  };
}

export function accountExportCompletionMessage(payload: {
  activity_complete?: boolean;
  payments_complete?: boolean;
  invoices_complete?: boolean;
  payout_destinations_complete?: boolean;
}): string {
  const missing: string[] = [];
  if (payload.activity_complete === false) missing.push("older account activity");
  if (payload.payments_complete === false) missing.push("older Zcash payments");
  if (payload.invoices_complete === false) missing.push("older Zcash invoices");
  if (payload.payout_destinations_complete === false) missing.push("older private payout records");
  if (!missing.length) return "Your Zerant data export is ready.";
  const items = missing.length === 1
    ? missing[0]
    : missing.length === 2
      ? `${missing[0]} and ${missing[1]}`
      : `${missing.slice(0, -1).join(", ")}, and ${missing[missing.length - 1]}`;
  return `Export ready. ${items[0].toUpperCase()}${items.slice(1)} are not included in this file; use the relevant Zerant history pages for paginated history.`;
}

export function AccountSettings({
  authenticated,
  backendAvailable,
  summary,
  initialPasskeys,
  initialSessions,
  initialZcashMethods,
  activeSection,
}: {
  authenticated: boolean;
  backendAvailable: boolean;
  summary: AccountSummary | null;
  initialPasskeys: PasskeyView[];
  initialSessions: AccountSessionView[];
  initialZcashMethods: LinkedZcashMethod[];
  activeSection?: "overview" | "access" | "sessions" | "data";
}) {
  const router = useRouter();
  const [confirmText, setConfirmText] = useState("");
  const [status, setStatus] = useState("");
  const [passkeyCount, setPasskeyCount] = useState(initialPasskeys.length);
  const [zcashMethodCount, setZcashMethodCount] = useState(initialZcashMethods.length);
  const [sessionCount, setSessionCount] = useState(initialSessions.length);
  const showAllSections = activeSection === undefined;
  const showSection = (section: "access" | "sessions" | "data") => showAllSections || activeSection === section;

  async function downloadExport() {
    setStatus("Preparing your Zerant data…");
    const response = await fetch("/api/zerant/account/export", {
      credentials: "same-origin",
      cache: "no-store",
    });
    if (!response.ok) {
      setStatus(
        response.status === 429
          ? "You’ve requested exports too frequently. Try again shortly."
          : "Your export could not be prepared.",
      );
      return;
    }

    const payload = (await response.json()) as {
      activity_complete?: boolean;
      payments_complete?: boolean;
      invoices_complete?: boolean;
      payout_destinations_complete?: boolean;
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = "zerant-account-export.json";
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
    setStatus(accountExportCompletionMessage(payload));
  }

  async function deleteAccount() {
    if (confirmText !== "DELETE") {
      setStatus("Type DELETE exactly to confirm account deletion.");
      return;
    }

    const response = await fetch("/api/zerant/account", {
      method: "DELETE",
      credentials: "same-origin",
    });
    if (!response.ok) {
      setStatus(
        response.status === 409
          ? "This account represents an issuer or verifier. Retire or transfer that organization role before deleting the account."
          : "Account deletion could not be completed.",
      );
      return;
    }

    router.push("/");
    router.refresh();
  }

  if (!backendAvailable) {
    return (
      <main id="main" className="account-page">
        <section className="account-hero">
          <p className="eyebrow">Account</p>
          <h1>Your account settings are temporarily unavailable.</h1>
        </section>
      </main>
    );
  }

  if (!authenticated || !summary) {
    return (
      <main id="main" className="account-page">
        <section className="account-hero">
          <p className="eyebrow">Account</p>
          <h1>Manage your Zerant data.</h1>
          <p>Connect to Zerant to export or manage your account.</p>
          <Link href="/vault" className="button">
            Connect to Zerant <span aria-hidden="true">→</span>
          </Link>
        </section>
      </main>
    );
  }

  const security = accountSecuritySummaryFromCounts(passkeyCount, zcashMethodCount, sessionCount);

  return (
    <main id="main" className="account-page">
      <section className="account-hero">
        <p className="eyebrow">Account</p>
        <h1>Your data, your decision.</h1>
        <p>
          Export your Zerant data whenever you want. Personal accounts can also be permanently
          deleted without contacting support.
        </p>
      </section>

      {activeSection !== undefined ? (
        <WorkspaceSectionNav
          activeHref={activeSection === "overview" ? "/account" : `/account/${activeSection}`}
          items={[
            { href: "/account", label: "Overview" },
            { href: "/account/access", label: "Access methods" },
            { href: "/account/sessions", label: "Sessions" },
            { href: "/account/data", label: "Data and deletion" },
          ]}
        />
      ) : null}

      <section className="account-security-overview" aria-label="Account security overview">
        <div className="account-security-heading">
          <div><p className="eyebrow">Access overview</p><h2>Know how your Zerant account can be opened.</h2></div>
          <span className={security.hasRedundantAccess ? "workspace-state ready" : "workspace-state"}><span />{security.hasRedundantAccess ? "Multiple sign-in methods" : "Single sign-in method"}</span>
        </div>
        <div className="account-security-grid">
          <div><strong>{security.accessMethods}</strong><span>sign-in method{security.accessMethods === 1 ? "" : "s"}</span></div>
          <div><strong>{security.passkeys}</strong><span>passkey{security.passkeys === 1 ? "" : "s"}</span></div>
          <div><strong>{security.zcashMethods}</strong><span>Zcash sign-in method{security.zcashMethods === 1 ? "" : "s"}</span></div>
          <div><strong>{security.activeSessions}</strong><span>active session{security.activeSessions === 1 ? "" : "s"}</span></div>
        </div>
        <p className="small muted">Zerant does not build a device, location, payment-address, balance, or wallet-history profile from these access methods.</p>
      </section>

      {activeSection === "overview" ? (
        <section className="workspace-route-cards" aria-label="Account sections">
          <Link href="/account/access"><strong>Access methods</strong><span>Manage passkeys and optional Zcash sign-in.</span></Link>
          <Link href="/account/sessions"><strong>Sessions</strong><span>Review and revoke active Zerant sessions.</span></Link>
          <Link href="/account/data"><strong>Data and deletion</strong><span>Export your data or remove your account.</span></Link>
        </section>
      ) : null}

      <section className={activeSection === "overview" ? "account-grid" : showAllSections ? "account-grid" : "account-grid account-grid-section"}>
        <article className="account-card">
          <p className="eyebrow">Your Zerant ID</p>
          <h2>{summary.zerant_id}</h2>
          <p className="muted">
            {summary.credential_count} private credential
            {summary.credential_count === 1 ? "" : "s"} currently stored.
          </p>
        </article>

        {summary.issuer_profile ? (
          <article className="account-card">
            <div className="account-role-heading"><p className="eyebrow">Issuer organization</p><span className={summary.issuer_retired ? "pill issuer-retired-pill" : "pill"}>{summary.issuer_retired ? "Retired" : "Active"}</span></div>
            <h2>{summary.issuer_profile}</h2>
            <p className="muted">
              Your role: {summary.issuer_role ?? "member"}. {summary.issuer_retired
                ? "The issuer is archive-only. If you own it, ownership still needs to be transferred before deleting your personal account."
                : "Organization permissions are managed from the issuer workspace."}
            </p>
            <Link href="/issuer" className="text-link">
              Open issuer workspace →
            </Link>
          </article>
        ) : null}

        {summary.verifier_profile ? (
          <article className="account-card">
            <div className="account-role-heading"><p className="eyebrow">Verifier organization</p><span className={summary.verifier_retired ? "pill issuer-retired-pill" : "pill"}>{summary.verifier_retired ? "Retired" : "Active"}</span></div>
            <h2>{summary.verifier_profile}</h2>
            <p className="muted">{summary.verifier_retired
              ? "This verifier is archive-only. Its historical records remain, but it no longer blocks deletion of your personal Zerant account."
              : "Verification requests, integrations, and verifier keys are managed from the verifier workspace."}</p>
            <Link href="/verifier" className="text-link">Open verifier workspace →</Link>
          </article>
        ) : null}

        {showSection("access") ? <PasskeyManager initialPasskeys={initialPasskeys} onCountChange={setPasskeyCount} /> : null}

        {showSection("access") ? <ZcashSignInManager initialMethods={initialZcashMethods} onCountChange={setZcashMethodCount} /> : null}

        {showSection("sessions") ? <SessionManager initialSessions={initialSessions} onCountChange={setSessionCount} /> : null}

        {showSection("data") ? <article className="account-card">
          <p className="eyebrow">Export</p>
          <h2>Take your Zerant data with you.</h2>
          <p className="muted">
            Download your private credentials, account activity, saved payment details, and active private payout destinations as a portable JSON file. Keep it private.
          </p>
          <Button onClick={downloadExport}>Download my data</Button>
        </article> : null}

        {showSection("data") ? <article className="account-card account-card-danger">
          <p className="eyebrow">Delete account</p>
          <h2>Remove your personal Zerant account.</h2>

          {summary.can_delete ? (
            <>
              <p className="muted">
                This permanently removes your personal Zerant ID, stored credentials, private payout records, sessions
                and account activity. This cannot be undone.
              </p>
              <label htmlFor="delete-confirm">Type DELETE to confirm</label>
              <input
                id="delete-confirm"
                value={confirmText}
                onChange={(event) => setConfirmText(event.target.value)}
                autoComplete="off"
              />
              <Button
                variant="secondary"
                disabled={confirmText !== "DELETE"}
                onClick={deleteAccount}
              >
                Delete my account
              </Button>
            </>
          ) : (
            <p className="muted">
              This account currently owns
              {summary.issuer_role === "owner" && summary.issuer_profile
                ? " issuer " + summary.issuer_profile
                : ""}
              {summary.issuer_role === "owner" && summary.issuer_profile && summary.verifier_profile
                ? " and"
                : ""}
              {summary.verifier_profile ? " verifier " + summary.verifier_profile : ""}.
              Active organizational ownership must be transferred before the account can be deleted. Retiring a verifier closes new work and removes that verifier as a deletion blocker; issuer ownership still requires transfer.
            </p>
          )}
        </article> : null}
      </section>

      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </main>
  );
}
