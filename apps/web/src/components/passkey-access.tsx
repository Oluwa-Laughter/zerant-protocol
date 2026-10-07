"use client";

import { useState } from "react";
import {
  browserSupportsWebAuthn,
  startAuthentication,
  startRegistration,
} from "@simplewebauthn/browser";
import { Button } from "@/components/ui/button";

type RegistrationOptionsJSON = Parameters<typeof startRegistration>[0]["optionsJSON"];
type AuthenticationOptionsJSON = Parameters<typeof startAuthentication>[0]["optionsJSON"];

type RegistrationStart = {
  zerant_id: string;
  public_key: {
    publicKey: RegistrationOptionsJSON;
  };
};

type AuthenticationStart = {
  public_key: {
    publicKey: AuthenticationOptionsJSON;
  };
};

function passkeyError(error: unknown): string {
  if (error instanceof TypeError) return "Zerant could not reach the service. Check your connection and try again.";
  const message = error instanceof Error ? error.message : "";
  if (/not allowed|cancel|abort/i.test(message)) {
    return "Passkey prompt closed or no discoverable passkey was available. Try again or use your Zerant ID below.";
  }
  if (/not found|no credential|no passkey/i.test(message)) return "No matching passkey was found on this device. Try your Zerant ID below.";
  if (/not supported/i.test(message)) {
    return "This browser or device does not support passkeys.";
  }
  return "The passkey request could not be completed. Try again.";
}

function shouldOfferIdFallback(error: unknown): boolean {
  if (error instanceof TypeError) return false;
  const message = error instanceof Error ? error.message : "";
  return /not allowed|cancel|abort|not found|no credential|no passkey/i.test(message);
}

function serverError(status: number, step: "start" | "finish"): string {
  if (status === 410) return "This sign-in attempt expired. Start again.";
  if (status === 429) return "Too many attempts. Wait a moment, then try again.";
  if (status >= 500) return "Zerant is temporarily unavailable. Try again shortly.";
  return step === "start"
    ? "Zerant could not start this passkey request. Try again."
    : "This passkey could not be verified. Start a new sign-in attempt.";
}

export function PasskeyAccess({ onConnected }: { onConnected?: () => void }) {
  const [zerantId, setZerantId] = useState("");
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const [showIdFallback, setShowIdFallback] = useState(false);

  async function createAccount() {
    if (busy) return;
    if (!browserSupportsWebAuthn()) {
      setStatus("This browser or device does not support passkeys.");
      return;
    }

    setBusy(true);
    try {
      setStatus("Creating a private Zerant account…");
      const startResponse = await fetch("/api/zerant/auth/passkey/register/start", {
        method: "POST",
        credentials: "same-origin",
        cache: "no-store",
      });
      if (!startResponse.ok) {
        setStatus(serverError(startResponse.status, "start"));
        return;
      }

      const started = (await startResponse.json()) as RegistrationStart;
      const credential = await startRegistration({
        optionsJSON: started.public_key.publicKey,
      });

      const finishResponse = await fetch("/api/zerant/auth/passkey/register/finish", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(credential),
      });
      if (!finishResponse.ok) {
        setStatus(serverError(finishResponse.status, "finish"));
        return;
      }

      setStatus("Passkey created. Your Zerant account is ready.");
      onConnected?.();
    } catch (error) {
      setStatus(passkeyError(error));
    } finally {
      setBusy(false);
    }
  }

  async function signInWithPasskey() {
    if (busy) return;
    if (!browserSupportsWebAuthn()) {
      setStatus("This browser or device does not support passkeys.");
      return;
    }

    setBusy(true);
    try {
      setStatus("Choose your Zerant passkey…");
      const startResponse = await fetch("/api/zerant/auth/passkey/discoverable/start", {
        method: "POST",
        credentials: "same-origin",
        cache: "no-store",
      });
      if (!startResponse.ok) {
        setStatus(serverError(startResponse.status, "start"));
        return;
      }
      const started = (await startResponse.json()) as AuthenticationStart;
      const credential = await startAuthentication({ optionsJSON: started.public_key.publicKey });
      const finishResponse = await fetch("/api/zerant/auth/passkey/discoverable/finish", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(credential),
      });
      if (!finishResponse.ok) {
        setStatus(serverError(finishResponse.status, "finish"));
        if (finishResponse.status === 401 || finishResponse.status === 404) setShowIdFallback(true);
        return;
      }
      setStatus("Signed in.");
      onConnected?.();
    } catch (error) {
      setStatus(passkeyError(error));
      if (shouldOfferIdFallback(error)) setShowIdFallback(true);
    } finally {
      setBusy(false);
    }
  }

  async function signInWithId() {
    if (busy) return;
    const normalized = zerantId.trim();
    if (!/^zr_[0-9a-f]{24}$/i.test(normalized)) {
      setStatus("Enter your Zerant ID first.");
      return;
    }
    if (!browserSupportsWebAuthn()) {
      setStatus("This browser or device does not support passkeys.");
      return;
    }

    setBusy(true);
    try {
      setStatus("Preparing your passkey sign-in…");
      const startResponse = await fetch("/api/zerant/auth/passkey/authenticate/start", {
        method: "POST",
        credentials: "same-origin",
        cache: "no-store",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ zerant_id: normalized }),
      });
      if (!startResponse.ok) {
        setStatus(startResponse.status === 404
          ? "No matching passkey was found for this Zerant ID. Check the ID or use another device."
          : serverError(startResponse.status, "start"));
        return;
      }

      const started = (await startResponse.json()) as AuthenticationStart;
      const credential = await startAuthentication({
        optionsJSON: started.public_key.publicKey,
      });

      const finishResponse = await fetch("/api/zerant/auth/passkey/authenticate/finish", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(credential),
      });
      if (!finishResponse.ok) {
        setStatus(serverError(finishResponse.status, "finish"));
        return;
      }

      setStatus("Signed in.");
      onConnected?.();
    } catch (error) {
      setStatus(passkeyError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="passkey-access">
      <div>
        <p className="eyebrow">Private account access</p>
        <h3>Continue with passkey</h3>
        <p className="small muted">
          New to Zerant? Create an account with your device passkey. No Zcash wallet is needed.
          Payments open in an external wallet only when you choose to pay.
        </p>
      </div>

      <div className="passkey-actions">
        <Button onClick={signInWithPasskey} disabled={busy}>
          {busy ? "Waiting for passkey…" : "Continue with passkey"}
        </Button>
        <Button variant="secondary" onClick={createAccount} disabled={busy}>Create a Zerant account</Button>
        <details
          className="passkey-id-fallback"
          open={showIdFallback}
          onToggle={(event) => setShowIdFallback(event.currentTarget.open)}
        >
          <summary>Use Zerant ID instead</summary>
          <div className="passkey-signin">
          <p className="small muted">Some valid passkeys are not discoverable until Zerant knows which account to check.</p>
          <label htmlFor="passkey-zerant-id">Already have an account? Enter your Zerant ID</label>
          <input
            id="passkey-zerant-id"
            value={zerantId}
            onChange={(event) => setZerantId(event.target.value)}
            placeholder="zr_..."
            autoComplete="username webauthn"
            spellCheck={false}
            aria-describedby="passkey-id-help"
            maxLength={27}
          />
          <Button
            variant="secondary"
            onClick={signInWithId}
            disabled={busy || !zerantId.trim()}
          >
            Continue with Zerant ID
          </Button>
          <p id="passkey-id-help" className="small muted passkey-id-help">Your ID appears in your Vault after account creation. Save it to sign in on another device. It is separate from your Zcash address.</p>
          </div>
        </details>
      </div>

      {status ? (
        <p className="vault-status neutral" role="status">
          {status}
        </p>
      ) : null}
    </div>
  );
}
