"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";

type Challenge = {
  domain: string;
  uri: string;
  version: number;
  chain: string;
  nonce: string;
  issued_at: string;
  expiration_time: string;
  statement: string;
  scopes: { required: Array<{ type: string }> };
};

export function ZcashConnect({ onConnected }: { onConnected?: () => void }) {
  const [status, setStatus] = useState("");

  async function begin() {
    setStatus("Creating a Zcash authentication challenge…");
    const response = await fetch("/api/zerant/auth/challenge?scopes=signin", {
      credentials: "same-origin",
      cache: "no-store",
    });
    if (!response.ok) {
      setStatus("Zcash authentication service is unavailable.");
      return;
    }
    const challenge = (await response.json()) as Challenge;
    const payload = {
      domain: challenge.domain,
      uri: challenge.uri,
      version: challenge.version,
      chain: challenge.chain,
      nonce: challenge.nonce,
      issued_at: challenge.issued_at,
      expiration_time: challenge.expiration_time,
      statement: challenge.statement,
      scopes: challenge.scopes,
    };
    const callback = `${window.location.origin}/api/zerant/auth/verify`;
    const link = `zecauth://${challenge.domain}?challenge=${encodeURIComponent(
      JSON.stringify(payload),
    )}&callback=${encodeURIComponent(callback)}`;
    setStatus("Opening your Zcash wallet. Approve the Zerant sign-in request there.");
    window.location.href = link;
  }

  async function check() {
    setStatus("Checking the server for your signed Zcash challenge…");
    const response = await fetch("/api/zerant/auth/session", {
      credentials: "same-origin",
      cache: "no-store",
    });
    if (!response.ok) {
      setStatus("No completed Zcash sign-in is ready yet.");
      return;
    }
    setStatus("Zcash identity authenticated.");
    onConnected?.();
  }

  return (
    <div className="zcash-connect">
      <div>
        <p className="eyebrow">Zcash-native authentication</p>
        <h2>Connect your Zcash identity.</h2>
        <p className="muted">
          Zerant uses a purpose-specific ZecAuth key for authentication. Your payment
          address and spending keys are not used for sign-in.
        </p>
      </div>
      <div className="vault-actions wrap">
        <Button onClick={begin}>Connect with Zcash</Button>
        <Button variant="secondary" onClick={check}>Check connection</Button>
      </div>
      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </div>
  );
}
