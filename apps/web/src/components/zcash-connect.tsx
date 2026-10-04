"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  buildZecAuthWalletUri,
  getPreferredInjectedZcashWallet,
  type ZecAuthWalletChallenge,
} from "@/lib/zcash-wallet";

type Challenge = ZecAuthWalletChallenge & {
  message: string;
};

async function createChallenge(): Promise<Challenge> {
  const response = await fetch("/api/zerant/auth/challenge?scopes=signin", {
    credentials: "same-origin",
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error("Zerant could not create a wallet sign-in request.");
  }
  return (await response.json()) as Challenge;
}

async function redeemSession(): Promise<void> {
  const response = await fetch("/api/zerant/auth/session", {
    credentials: "same-origin",
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error("No completed wallet approval is ready yet.");
  }
}

function connectionErrorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : "";

  if (/rejected|denied|cancel/i.test(message)) {
    return "Wallet connection was cancelled.";
  }
  if (/no response.*background|background.*no response/i.test(message)) {
    return "Your wallet is installed but did not respond. Open and unlock it, then try again.";
  }
  if (/locked/i.test(message)) {
    return "Unlock your wallet, then try again.";
  }
  if (message) {
    return message;
  }
  return "The Zcash wallet connection was not completed.";
}

export function ZcashConnect({ onConnected }: { onConnected?: () => void }) {
  const [status, setStatus] = useState("");
  const [connecting, setConnecting] = useState(false);

  async function connectInjectedWallet() {
    if (connecting) return;
    setConnecting(true);

    try {
      const wallet = getPreferredInjectedZcashWallet("identitySigning");
      if (!wallet || !wallet.signIdentityChallenge) {
        setStatus(
          "No compatible injected Zcash wallet was detected. Use the wallet-app option instead.",
        );
        return;
      }

      setStatus("Connecting to your Zcash wallet…");
      await wallet.ensureConnection();

      setStatus("Approve the Zerant sign-in request in your wallet…");
      const challenge = await createChallenge();
      const signed = await wallet.signIdentityChallenge(challenge.message);

      const verify = await fetch("/api/zerant/auth/wallet/verify", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          pubkey: signed.pubkey,
          signature: signed.signature,
          message: challenge.message,
          granted: ["auth"],
          signing_mode: signed.signingMode,
        }),
      });

      if (!verify.ok) {
        setStatus("Zerant could not verify the wallet sign-in.");
        return;
      }

      await redeemSession();
      setStatus("Wallet connected.");
      onConnected?.();
    } catch (error) {
      setStatus(connectionErrorMessage(error));
    } finally {
      setConnecting(false);
    }
  }

  async function openWalletApp() {
    try {
      setStatus("Creating a Zcash wallet sign-in request…");
      const challenge = await createChallenge();
      const callback = window.location.origin + "/api/zerant/auth/verify";
      const uri = buildZecAuthWalletUri(challenge, callback);

      setStatus("Opening your Zcash wallet. Approve the Zerant sign-in request there.");
      window.location.assign(uri);
    } catch (error) {
      setStatus(connectionErrorMessage(error));
    }
  }

  async function checkWalletApproval() {
    try {
      setStatus("Checking for your approved wallet response…");
      await redeemSession();
      setStatus("Wallet connected.");
      onConnected?.();
    } catch (error) {
      setStatus(connectionErrorMessage(error));
    }
  }

  return (
    <div className="zcash-connect">
      <div>
        <p className="eyebrow">Connect a Zcash wallet</p>
        <h2>Use the wallet you already trust.</h2>
        <p className="muted">
          Zerant keeps wallet choice separate from your trust profile. Your payment address,
          balance and transaction history are not used as your Zerant identity.
        </p>
      </div>

      <div className="wallet-connect-options">
        <article className="wallet-connect-option">
          <div>
            <span className="eyebrow">Browser wallet</span>
            <h3>Connect an installed wallet</h3>
            <p className="small muted">
              If an installed Zcash wallet exposes compatible browser signing, Zerant can connect
              to it directly.
            </p>
          </div>
          <Button onClick={connectInjectedWallet} disabled={connecting}>
            {connecting ? "Connecting…" : "Connect browser wallet"}
          </Button>
        </article>

        <article className="wallet-connect-option">
          <div>
            <span className="eyebrow">Wallet app</span>
            <h3>Open the request in your wallet</h3>
            <p className="small muted">
              Wallet apps that support Zcash authentication handoff can approve the same Zerant
              sign-in request without a browser extension.
            </p>
          </div>
          <div className="vault-actions wrap">
            <Button variant="secondary" onClick={openWalletApp}>
              Open wallet app
            </Button>
            <Button variant="secondary" onClick={checkWalletApproval}>
              Check approval
            </Button>
          </div>
        </article>
      </div>

      <p className="small muted wallet-compatibility-note">
        Wallet support is capability-based. Zerant does not require one wallet brand, and payment
        requests use standard Zcash wallet handoff whenever possible.
      </p>

      {status ? (
        <p className="vault-status neutral" role="status">
          {status}
        </p>
      ) : null}
    </div>
  );
}
