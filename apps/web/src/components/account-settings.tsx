"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { PasskeyManager, type PasskeyView } from "@/components/passkey-manager";

export type AccountSummary = {
  zerant_id: string;
  credential_count: number;
  passkey_count: number;
  issuer_profile: string | null;
  issuer_role: string | null;
  verifier_profile: string | null;
  can_delete: boolean;
};

export function AccountSettings({
  authenticated,
  backendAvailable,
  summary,
  initialPasskeys,
}: {
  authenticated: boolean;
  backendAvailable: boolean;
  summary: AccountSummary | null;
  initialPasskeys: PasskeyView[];
}) {
  const router = useRouter();
  const [confirmText, setConfirmText] = useState("");
  const [status, setStatus] = useState("");

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

    const payload = await response.json();
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
    setStatus("Your Zerant data export is ready.");
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

      <section className="account-grid">
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
            <p className="eyebrow">Issuer organization</p>
            <h2>{summary.issuer_profile}</h2>
            <p className="muted">
              Your role: {summary.issuer_role ?? "member"}. Organization permissions are managed
              from the issuer workspace.
            </p>
            <Link href="/issuer" className="text-link">
              Open issuer workspace →
            </Link>
          </article>
        ) : null}


        <PasskeyManager initialPasskeys={initialPasskeys} />

        <article className="account-card">
          <p className="eyebrow">Export</p>
          <h2>Take your Zerant data with you.</h2>
          <p className="muted">
            Download your private credentials and account activity as a portable JSON file.
          </p>
          <Button onClick={downloadExport}>Download my data</Button>
        </article>

        <article className="account-card account-card-danger">
          <p className="eyebrow">Delete account</p>
          <h2>Remove your personal Zerant account.</h2>

          {summary.can_delete ? (
            <>
              <p className="muted">
                This permanently removes your personal Zerant ID, stored credentials, sessions
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
              Organizational ownership must be transferred before the account can be deleted.
            </p>
          )}
        </article>
      </section>

      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </main>
  );
}
