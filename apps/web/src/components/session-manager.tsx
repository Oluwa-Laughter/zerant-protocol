"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";

export type AccountSessionView = {
  id: string;
  auth_method: string;
  current: boolean;
  created_at: string;
  last_seen_at: string;
  expires_at: string;
};

function accessLabel(method: string): string {
  if (method === "passkey") return "Passkey";
  if (method === "zcash") return "Zcash";
  return "Zerant session";
}

export function SessionManager({
  initialSessions,
}: {
  initialSessions: AccountSessionView[];
}) {
  const router = useRouter();
  const [sessions, setSessions] = useState(initialSessions);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);

  async function revokeSession(session: AccountSessionView) {
    if (busy) return;
    setBusy(true);
    try {
      const response = await fetch(
        session.current
          ? "/api/zerant/session"
          : "/api/zerant/account/sessions/" + encodeURIComponent(session.id),
        {
          method: "DELETE",
          credentials: "same-origin",
        },
      );

      if (!response.ok) {
        setStatus(
          response.status === 403
            ? "Sign in again before changing active sessions."
            : "That session could not be signed out.",
        );
        return;
      }

      if (session.current) {
        router.push("/");
        router.refresh();
        return;
      }

      setSessions((current) => current.filter((item) => item.id !== session.id));
      setStatus("Session signed out.");
    } finally {
      setBusy(false);
    }
  }

  async function revokeOthers() {
    if (busy) return;
    setBusy(true);
    try {
      const response = await fetch("/api/zerant/account/sessions/revoke-others", {
        method: "POST",
        credentials: "same-origin",
      });

      if (!response.ok) {
        setStatus(
          response.status === 403
            ? "Sign in again before signing out other sessions."
            : "Other sessions could not be signed out.",
        );
        return;
      }

      setSessions((current) => current.filter((item) => item.current));
      setStatus("All other sessions have been signed out.");
    } finally {
      setBusy(false);
    }
  }

  const otherCount = sessions.filter((session) => !session.current).length;

  return (
    <article className="account-card account-sessions">
      <div className="account-session-heading">
        <div>
          <p className="eyebrow">Active sessions</p>
          <h2>Control where your Zerant account is signed in.</h2>
        </div>
        <span className="pill">
          {sessions.length} active
        </span>
      </div>

      <p className="muted">
        Zerant shows only access method and activity time. It does not create a location or
        browser-fingerprint history for your account.
      </p>

      <div className="account-session-list">
        {sessions.map((session) => (
          <div className="account-session-row" key={session.id}>
            <div>
              <div className="account-session-title">
                <strong>{accessLabel(session.auth_method)}</strong>
                {session.current ? <span className="request-status approved">This session</span> : null}
              </div>
              <span className="small muted">
                Started {new Date(session.created_at).toLocaleString()} · last active{" "}
                {new Date(session.last_seen_at).toLocaleString()}
              </span>
            </div>
            <Button
              variant="secondary"
              disabled={busy}
              onClick={() => revokeSession(session)}
            >
              {session.current ? "Sign out" : "Sign out session"}
            </Button>
          </div>
        ))}
      </div>

      {otherCount > 0 ? (
        <Button variant="secondary" disabled={busy} onClick={revokeOthers}>
          Sign out all other sessions
        </Button>
      ) : null}

      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </article>
  );
}
