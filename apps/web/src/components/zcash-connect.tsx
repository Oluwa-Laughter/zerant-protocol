"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { getInjectedZcashWallet } from "@/lib/zcash-wallet";

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
    throw new Error("The signed wallet request could not be completed.");
  }
}

export function ZcashConnect({ onConnected }: { onConnected?: () => void }) {
  const [status, setStatus] = useState("");

  async function connectBrowserWallet() {
    try {
      const wallet = getInjectedZcashWallet();
      if (!wallet) {
        setStatus(
          "No compatible Zcash browser wallet was detected. Use the wallet-app option instead.",
        );
        return;
      }

      setStatus("Waiting for your wallet connection approval…");
      const connection = await wallet.connect();
      if (!connection.shieldedAddress) {
        setStatus("This wallet did not provide a shielded Zcash address.");
        return;
      }

      setStatus("Approve the private Zerant sign-in message in your wallet…");
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
        setStatus("The wallet signature could not be verified.");
        return;
      }

      await redeemSession();
      setStatus("Zcash wallet connected to Zerant.");
      onConnected?.();
    } catch (error) {
      setStatus(
        error instanceof Error
          ? error.message
          : "The Zcash wallet connection was not completed.",
      );
    }
  }

  async function beginWalletApp() {
    try {
      setStatus("Creating a private Zcash wallet sign-in request…");
      const challenge = await createChallenge();
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
      const callback = window.location.origin + "/api/zerant/auth/verify";
      const link =
        "zecauth://" +
        challenge.domain +
        "?challenge=" +
        encodeURIComponent(JSON.stringify(payload)) +
        "&callback=" +
        encodeURIComponent(callback);

      setStatus("Opening a compatible Zcash wallet. Approve the Zerant sign-in request there.");
      window.location.href = link;
    } catch (error) {
      setStatus(
        error instanceof Error
          ? error.message
          : "The wallet-app sign-in request could not be created.",
      );
    }
  }

  async function checkWalletApp() {
    try {
      setStatus("Checking for your approved wallet response…");
      await redeemSession();
      setStatus("Zcash wallet connected to Zerant.");
      onConnected?.();
    } catch {
      setStatus("No completed wallet approval is ready yet.");
    }
  }

  return (
    <div className="zcash-connect">
      <div>
        <p className="eyebrow">Connect a Zcash wallet</p>
        <h2>Use the wallet you already trust.</h2>
        <p className="muted">
          Zerant connects to compatible Zcash wallets without using your payment address,
          balance, or transaction history as your identity.
        </p>
      </div>

      <div className="wallet-connect-options">
        <article className="wallet-connect-option">
          <div>
            <span className="eyebrow">Browser wallet</span>
            <h3>Connect an installed Zcash wallet</h3>
            <p className="small muted">
              Zerant detects compatible injected wallets and asks only for connection and a
              private identity signature.
            </p>
          </div>
          <Button onClick={connectBrowserWallet}>Connect browser wallet</Button>
        </article>

        <article className="wallet-connect-option">
          <div>
            <span className="eyebrow">Wallet app</span>
            <h3>Use another compatible Zcash wallet</h3>
            <p className="small muted">
              Open the request in a wallet app that supports Zcash authentication handoff.
            </p>
          </div>
          <div className="vault-actions wrap">
            <Button variant="secondary" onClick={beginWalletApp}>
              Open wallet app
            </Button>
            <Button variant="secondary" onClick={checkWalletApp}>
              Check approval
            </Button>
          </div>
        </article>
      </div>

      {status ? (
        <p className="vault-status neutral" role="status">
          {status}
        </p>
      ) : null}
    </div>
  );
}
