"use client";

import { useState } from "react";
import {
  browserSupportsWebAuthn,
  startRegistration,
} from "@simplewebauthn/browser";
import { Button } from "@/components/ui/button";

type RegistrationOptionsJSON = Parameters<typeof startRegistration>[0]["optionsJSON"];

export type PasskeyView = {
  id: string;
  last_used_at: string | null;
  created_at: string;
  updated_at: string;
};

type RegistrationStart = {
  zerant_id: string;
  public_key: {
    publicKey: RegistrationOptionsJSON;
  };
};

function passkeyError(error: unknown): string {
  const message = error instanceof Error ? error.message : "";
  if (/not allowed|cancel|abort/i.test(message)) {
    return "Passkey setup was cancelled.";
  }
  if (/not supported/i.test(message)) {
    return "This browser or device does not support passkeys.";
  }
  return message || "Passkey setup could not be completed.";
}

export function PasskeyManager({
  initialPasskeys,
}: {
  initialPasskeys: PasskeyView[];
}) {
  const [passkeys, setPasskeys] = useState(initialPasskeys);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const [pendingRemoval, setPendingRemoval] = useState<string | null>(null);

  async function addPasskey() {
    if (busy) return;
    if (!browserSupportsWebAuthn()) {
      setStatus("This browser or device does not support passkeys.");
      return;
    }

    setBusy(true);
    try {
      const startResponse = await fetch("/api/zerant/account/passkeys/register/start", {
        method: "POST",
        credentials: "same-origin",
        cache: "no-store",
      });
      if (!startResponse.ok) {
        setStatus(
          startResponse.status === 403
            ? "Sign in again before changing your account access methods."
            : startResponse.status === 409
              ? "This account already has the maximum number of passkeys."
              : "Zerant could not start passkey setup.",
        );
        return;
      }

      const started = (await startResponse.json()) as RegistrationStart;
      const credential = await startRegistration({
        optionsJSON: started.public_key.publicKey,
      });

      const finishResponse = await fetch("/api/zerant/account/passkeys/register/finish", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(credential),
      });
      if (!finishResponse.ok) {
        setStatus(
          finishResponse.status === 403
            ? "Your security session expired. Sign in again before adding this passkey."
            : finishResponse.status === 409
              ? "This passkey is already connected or the account reached its passkey limit."
              : "Passkey setup could not be completed.",
        );
        return;
      }

      const created = (await finishResponse.json()) as PasskeyView;
      setPasskeys((current) => [...current, created]);
      setStatus("Passkey added to your Zerant account.");
    } catch (error) {
      setStatus(passkeyError(error));
    } finally {
      setBusy(false);
    }
  }

  async function removePasskey(id: string) {
    if (busy) return;
    setBusy(true);
    try {
      const response = await fetch(
        "/api/zerant/account/passkeys/" + encodeURIComponent(id),
        {
          method: "DELETE",
          credentials: "same-origin",
        },
      );
      if (!response.ok) {
        setStatus(
          response.status === 403
            ? "Sign in again before removing a passkey."
            : response.status === 409
              ? "Keep at least one sign-in method connected before removing this passkey."
              : "Passkey could not be removed.",
        );
        return;
      }

      setPasskeys((current) => current.filter((item) => item.id !== id));
      setPendingRemoval(null);
      setStatus("Passkey removed.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <article className="account-card account-passkeys">
      <div className="account-passkey-heading">
        <div>
          <p className="eyebrow">Passkeys</p>
          <h2>Keep another secure way back into Zerant.</h2>
        </div>
        <span className="pill">
          {passkeys.length} passkey{passkeys.length === 1 ? "" : "s"}
        </span>
      </div>

      <p className="muted">
        Add passkeys for devices you trust. Zerant blocks removal when it would leave your
        account without any usable sign-in method.
      </p>

      <div className="account-passkey-list">
        {passkeys.length ? (
          passkeys.map((passkey, index) => (
            <div className="account-passkey-row" key={passkey.id}>
              <div>
                <strong>Passkey {index + 1}</strong>
                <span className="small muted">
                  Added {new Date(passkey.created_at).toLocaleDateString()}
                  {passkey.last_used_at
                    ? " · last used " + new Date(passkey.last_used_at).toLocaleDateString()
                    : " · not used yet"}
                </span>
              </div>
              {pendingRemoval === passkey.id ? (
                <div className="account-access-confirm">
                  <span className="small muted">Remove this passkey?</span>
                  <div className="vault-actions wrap">
                    <Button variant="secondary" disabled={busy} onClick={() => void removePasskey(passkey.id)}>Confirm</Button>
                    <Button variant="secondary" disabled={busy} onClick={() => setPendingRemoval(null)}>Cancel</Button>
                  </div>
                </div>
              ) : (
                <Button variant="secondary" disabled={busy} onClick={() => setPendingRemoval(passkey.id)}>Remove</Button>
              )}
            </div>
          ))
        ) : (
          <p className="muted">No passkey is connected to this account yet.</p>
        )}
      </div>

      <Button disabled={busy || passkeys.length >= 10} onClick={addPasskey}>
        {busy ? "Working…" : "Add a passkey"}
      </Button>

      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </article>
  );
}
