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
  const message = error instanceof Error ? error.message : "";
  if (/not allowed|cancel|abort/i.test(message)) {
    return "Passkey request was cancelled.";
  }
  if (/not supported/i.test(message)) {
    return "This browser or device does not support passkeys.";
  }
  return message || "The passkey request could not be completed.";
}

export function PasskeyAccess({ onConnected }: { onConnected?: () => void }) {
  const [zerantId, setZerantId] = useState("");
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);

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
        setStatus("Zerant could not start passkey setup.");
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
        setStatus("Zerant could not finish passkey setup.");
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
        setStatus("Zerant could not start passkey sign-in.");
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
        setStatus("This passkey could not sign in. Try your Zerant ID below if your device does not offer an account passkey.");
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
        setStatus("No passkey-enabled Zerant account was found for that Zerant ID.");
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
        setStatus("Passkey sign-in could not be verified.");
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
        <p className="eyebrow">Recommended access</p>
        <h3>Get started with a passkey</h3>
        <p className="small muted">
          New to Zerant? Create an account with your device passkey. No Zcash wallet is needed.
          You can connect a payment wallet later in the Zcash workspace.
        </p>
      </div>

      <div className="passkey-actions">
        <Button onClick={createAccount} disabled={busy}>
          {busy ? "Working…" : "Create Zerant account"}
        </Button>
        <Button variant="secondary" onClick={signInWithPasskey} disabled={busy}>
          Sign in with passkey
        </Button>
        <details className="passkey-id-fallback">
          <summary>Use your Zerant ID to sign in</summary>
          <div className="passkey-signin">
          <label htmlFor="passkey-zerant-id">Already have an account? Enter your Zerant ID</label>
          <input
            id="passkey-zerant-id"
            value={zerantId}
            onChange={(event) => setZerantId(event.target.value)}
            placeholder="zr_..."
            autoComplete="username webauthn"
            spellCheck={false}
            aria-describedby="passkey-id-help"
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
