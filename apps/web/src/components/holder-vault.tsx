"use client";

import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  createRecordId,
  createVault,
  recordIds,
  removeRecord,
  type VaultEnvelopeV1,
  type VaultSession,
  unlockVault,
  VaultError,
} from "@/lib/vault";
import { deleteVault, loadVault, saveVault } from "@/lib/vault-storage";

type Status = { kind: "neutral" | "success" | "error"; message: string };

export function HolderVault() {
  const [envelope, setEnvelope] = useState<VaultEnvelopeV1 | null>(null);
  const [session, setSession] = useState<VaultSession | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [recordInput, setRecordInput] = useState("");
  const [status, setStatus] = useState<Status>({
    kind: "neutral",
    message: "Checking this browser for an existing local vault…",
  });
  const [preview, setPreview] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [deleteArmed, setDeleteArmed] = useState(false);

  const ids = useMemo(() => (envelope ? recordIds(envelope) : []), [envelope]);
  const selectedRecordId =
    selectedId && ids.includes(selectedId) ? selectedId : (ids[0] ?? null);
  const unlocked = Boolean(session && !session.locked);

  useEffect(() => {
    let active = true;
    void loadVault()
      .then((stored) => {
        if (!active) return;
        setEnvelope(stored);
        setStatus({
          kind: "neutral",
          message: stored
            ? "Encrypted vault found. It is locked."
            : "No local vault exists in this browser yet.",
        });
      })
      .catch(() => {
        if (active) {
          setStatus({ kind: "error", message: "Local vault storage is unavailable." });
        }
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!session) return;

    const lock = (message: string) => {
      session.lock();
      setSession(null);
      setPreview(null);
      setPassphrase("");
      setRecordInput("");
      setStatus({ kind: "neutral", message });
    };

    let timer = window.setTimeout(
      () => lock("Vault auto-locked after five minutes of inactivity."),
      5 * 60 * 1000,
    );

    const reset = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(
        () => lock("Vault auto-locked after five minutes of inactivity."),
        5 * 60 * 1000,
      );
    };
    const visibility = () => {
      if (document.hidden) lock("Vault auto-locked when this tab became hidden.");
    };

    window.addEventListener("pointerdown", reset, { passive: true });
    window.addEventListener("keydown", reset);
    window.addEventListener("focus", reset);
    document.addEventListener("visibilitychange", visibility);

    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("pointerdown", reset);
      window.removeEventListener("keydown", reset);
      window.removeEventListener("focus", reset);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [session]);

  async function handleCreate() {
    try {
      const created = await createVault(passphrase);
      await saveVault(created.envelope);
      setEnvelope(created.envelope);
      setSession(created.session);
      setPassphrase("");
      setStatus({
        kind: "success",
        message: "Encrypted vault created locally and unlocked for this tab.",
      });
    } catch (error) {
      setStatus({
        kind: "error",
        message: error instanceof VaultError ? error.message : "Vault could not be created.",
      });
    }
  }

  async function handleUnlock() {
    if (!envelope) return;
    try {
      const unlockedSession = await unlockVault(envelope, passphrase);
      setSession(unlockedSession);
      setPassphrase("");
      setStatus({ kind: "success", message: "Vault unlocked locally for this tab." });
    } catch {
      setPassphrase("");
      setStatus({ kind: "error", message: "Vault could not be unlocked." });
    }
  }

  function handleLock() {
    session?.lock();
    setSession(null);
    setPreview(null);
    setPassphrase("");
    setRecordInput("");
    setStatus({ kind: "neutral", message: "Vault locked." });
  }

  async function handleEncryptRecord() {
    if (!envelope || !session) return;

    let parsed: unknown;
    try {
      parsed = JSON.parse(recordInput);
    } catch {
      setStatus({ kind: "error", message: "Record must be valid JSON." });
      return;
    }

    try {
      const recordId = createRecordId();
      const updated = await session.sealRecord(envelope, recordId, parsed);
      await saveVault(updated);
      setEnvelope(updated);
      setSelectedId(recordId);
      setRecordInput("");
      setPreview(null);
      setStatus({
        kind: "success",
        message: "Record encrypted and persisted locally.",
      });
    } catch {
      setStatus({ kind: "error", message: "The record could not be encrypted." });
    }
  }

  async function handleDecryptSelected() {
    if (!envelope || !session || !selectedRecordId) return;
    try {
      const value = await session.openRecord(envelope, selectedRecordId);
      setPreview(JSON.stringify(value, null, 2));
      setStatus({
        kind: "neutral",
        message: "Selected record decrypted in memory. Nothing was transmitted.",
      });
    } catch {
      setStatus({ kind: "error", message: "The selected record could not be decrypted." });
    }
  }

  async function handleRemoveSelected() {
    if (!envelope || !session || !selectedRecordId) return;
    try {
      const updated = removeRecord(envelope, selectedRecordId);
      await saveVault(updated);
      setEnvelope(updated);
      setSelectedId(null);
      setPreview(null);
      setStatus({ kind: "neutral", message: "Selected record removed from the local vault." });
    } catch {
      setStatus({ kind: "error", message: "The selected record could not be removed." });
    }
  }

  async function handleDeleteVault() {
    if (!deleteArmed) {
      setDeleteArmed(true);
      setStatus({
        kind: "neutral",
        message: "Deletion is armed. Press Delete local vault again to confirm.",
      });
      return;
    }

    try {
      session?.lock();
      await deleteVault();
      setSession(null);
      setEnvelope(null);
      setSelectedId(null);
      setPreview(null);
      setRecordInput("");
      setDeleteArmed(false);
      setPassphrase("");
      setStatus({ kind: "neutral", message: "Local vault deleted from this browser." });
    } catch {
      setStatus({ kind: "error", message: "The local vault could not be deleted." });
    }
  }

  return (
    <main id="main" className="vault-page">
      <section className="vault-hero">
        <p className="eyebrow">Holder vault · local browser boundary</p>
        <h1>Private evidence stays with the holder.</h1>
        <p>
          Zerant encrypts holder-provided JSON locally with Web Crypto and persists ciphertext
          in IndexedDB. The browser vault does not connect to a wallet or upload records.
        </p>
      </section>

      <div className="vault-warning" role="note">
        <strong>Credential and evidence storage only.</strong>
        <span>
          Do not enter Zcash seed phrases, spending keys, wallet passwords, PCZT artifacts,
          FROST shares, or recovery secrets.
        </span>
      </div>

      <section className="vault-grid">
        <article className="vault-card">
          <div className="vault-card-heading">
            <div>
              <p className="eyebrow">Access</p>
              <h2>{envelope ? (unlocked ? "Unlocked" : "Locked") : "Create local vault"}</h2>
            </div>
            <span className={unlocked ? "vault-state unlocked" : "vault-state"}>
              {unlocked ? "Local session active" : "No plaintext session"}
            </span>
          </div>

          {!unlocked ? (
            <div className="vault-form">
              <label htmlFor="vault-passphrase">Local vault passphrase</label>
              <input
                id="vault-passphrase"
                type="password"
                autoComplete={envelope ? "current-password" : "new-password"}
                value={passphrase}
                onChange={(event) => setPassphrase(event.target.value)}
                placeholder="12+ characters"
              />
              <p className="small muted">
                Zerant does not persist this passphrase and cannot recover it.
              </p>
              <Button onClick={envelope ? handleUnlock : handleCreate}>
                {envelope ? "Unlock local vault" : "Create encrypted vault"}
              </Button>
            </div>
          ) : (
            <div className="vault-actions">
              <Button variant="secondary" onClick={handleLock}>Lock now</Button>
              <span className="small muted">The local session auto-locks on inactivity or hidden tab.</span>
            </div>
          )}

          <p className={`vault-status ${status.kind}`} role="status" aria-live="polite">
            {status.message}
          </p>
        </article>

        <article className="vault-card">
          <div className="vault-card-heading">
            <div>
              <p className="eyebrow">Encrypted records</p>
              <h2>{ids.length} local record{ids.length === 1 ? "" : "s"}</h2>
            </div>
            <span className="pill">IndexedDB only</span>
          </div>

          {unlocked ? (
            <div className="vault-form">
              <label htmlFor="vault-record">Credential or private evidence JSON</label>
              <textarea
                id="vault-record"
                value={recordInput}
                onChange={(event) => setRecordInput(event.target.value)}
                spellCheck={false}
                rows={8}
                placeholder="Paste the JSON you want to encrypt locally"
              />
              <Button disabled={!recordInput.trim()} onClick={handleEncryptRecord}>
                Encrypt record
              </Button>
            </div>
          ) : (
            <p className="muted">Unlock the local vault to add or inspect encrypted records.</p>
          )}

          <div className="vault-record-list">
            {ids.length ? (
              ids.map((id) => (
                <button
                  type="button"
                  className={selectedRecordId === id ? "vault-record selected" : "vault-record"}
                  key={id}
                  onClick={() => {
                    setSelectedId(id);
                    setPreview(null);
                  }}
                  aria-pressed={selectedRecordId === id}
                >
                  <span className="mono">{id}</span>
                  <span>opaque ID · ciphertext persisted</span>
                </button>
              ))
            ) : (
              <p className="muted">No encrypted records.</p>
            )}
          </div>

          <div className="vault-actions wrap">
            <Button
              variant="secondary"
              disabled={!unlocked || !selectedRecordId}
              onClick={handleDecryptSelected}
            >
              Decrypt selected record
            </Button>
            <Button
              variant="secondary"
              disabled={!unlocked || !selectedRecordId}
              onClick={handleRemoveSelected}
            >
              Remove selected record
            </Button>
          </div>

          {preview ? (
            <pre className="vault-preview" aria-label="Decrypted local record">
              {preview}
            </pre>
          ) : null}
        </article>
      </section>

      <section className="vault-security">
        <div>
          <p className="eyebrow">Security boundary</p>
          <h2>Encryption is not anonymity.</h2>
        </div>
        <div className="vault-security-list">
          <p><strong>Implemented:</strong> AES-256-GCM, fresh IVs, authenticated opaque record IDs, PBKDF2-HMAC-SHA-256, wrapped random data key and IndexedDB persistence.</p>
          <p><strong>Residual risk:</strong> a compromised browser, extension, device or malicious same-origin script can read an unlocked vault.</p>
          <p><strong>Separate authority:</strong> wallet spending authority and threshold-signing secrets remain outside this browser vault.</p>
        </div>
      </section>

      <section className="vault-danger">
        <div>
          <p className="eyebrow">Local deletion</p>
          <h2>Remove this browser vault.</h2>
          <p className="muted">Deletion removes the encrypted IndexedDB record from this browser. There is no Zerant server copy.</p>
        </div>
        <Button variant="secondary" onClick={handleDeleteVault}>
          {deleteArmed ? "Delete local vault now" : "Delete local vault"}
        </Button>
      </section>
    </main>
  );
}
