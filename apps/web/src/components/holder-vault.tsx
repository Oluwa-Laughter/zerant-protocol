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

const SAMPLE_RECORD = {
  schema: "zerant.local-demo-record.v0.1",
  context: "freelancer",
  claim: "service.completed",
  value: "completed",
  note: "Synthetic local fixture only. No wallet or identity data.",
};

export function HolderVault() {
  const [envelope, setEnvelope] = useState<VaultEnvelopeV1 | null>(null);
  const [session, setSession] = useState<VaultSession | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [status, setStatus] = useState<Status>({
    kind: "neutral",
    message: "Checking this browser for an existing local vault…",
  });
  const [preview, setPreview] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [deleteArmed, setDeleteArmed] = useState(false);
  const ids = useMemo(() => (envelope ? recordIds(envelope) : []), [envelope]);
  const selectedRecordId = selectedId && ids.includes(selectedId) ? selectedId : (ids[0] ?? null);

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
        if (!active) return;
        setStatus({ kind: "error", message: "Local vault storage is unavailable." });
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!session) return;

    let timer = window.setTimeout(() => {
      session.lock();
      setSession(null);
      setPreview(null);
      setPassphrase("");
      setStatus({ kind: "neutral", message: "Vault auto-locked after five minutes of inactivity." });
    }, 5 * 60 * 1000);

    const reset = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        session.lock();
        setSession(null);
        setPreview(null);
        setPassphrase("");
        setStatus({ kind: "neutral", message: "Vault auto-locked after five minutes of inactivity." });
      }, 5 * 60 * 1000);
    };
    const visibility = () => {
      if (!document.hidden) return;
      session.lock();
      setSession(null);
      setPreview(null);
      setPassphrase("");
      setStatus({ kind: "neutral", message: "Vault auto-locked when this tab became hidden." });
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

  function clearPassphrase() {
    setPassphrase("");
  }

  async function handleCreate() {
    try {
      const created = await createVault(passphrase);
      await saveVault(created.envelope);
      setEnvelope(created.envelope);
      setSession(created.session);
      setPreview(null);
      clearPassphrase();
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
      const unlocked = await unlockVault(envelope, passphrase);
      setSession(unlocked);
      setPreview(null);
      clearPassphrase();
      setStatus({ kind: "success", message: "Vault unlocked locally for this tab." });
    } catch {
      clearPassphrase();
      setStatus({ kind: "error", message: "Vault could not be unlocked." });
    }
  }

  function handleLock() {
    session?.lock();
    setSession(null);
    setPreview(null);
    clearPassphrase();
    setStatus({ kind: "neutral", message: "Vault locked. Accessible key references were released." });
  }

  async function handleAddSample() {
    if (!envelope || !session) return;
    try {
      const recordId = createRecordId();
      const updated = await session.sealRecord(envelope, recordId, SAMPLE_RECORD);
      await saveVault(updated);
      setEnvelope(updated);
      setSelectedId(recordId);
      setPreview(null);
      setStatus({
        kind: "success",
        message: "Synthetic private record encrypted and persisted in IndexedDB.",
      });
    } catch {
      setStatus({ kind: "error", message: "The local record could not be encrypted." });
    }
  }

  async function handleReadSample() {
    if (!envelope || !session || !selectedRecordId) return;
    try {
      const value = await session.openRecord(envelope, selectedRecordId);
      setPreview(JSON.stringify(value, null, 2));
      setStatus({
        kind: "neutral",
        message: "Synthetic record decrypted in memory. Nothing was transmitted.",
      });
    } catch {
      setStatus({ kind: "error", message: "The local record could not be decrypted." });
    }
  }

  async function handleRemoveSample() {
    if (!envelope || !session || !selectedRecordId) return;
    try {
      const updated = removeRecord(envelope, selectedRecordId);
      await saveVault(updated);
      setEnvelope(updated);
      setSelectedId(null);
      setPreview(null);
      setStatus({ kind: "neutral", message: "Synthetic record removed from the local vault." });
    } catch {
      setStatus({ kind: "error", message: "The local record could not be removed." });
    }
  }

  async function handleDeleteVault() {
    if (!deleteArmed) {
      setDeleteArmed(true);
      setStatus({
        kind: "neutral",
        message: "Deletion is armed. Press Delete local vault again to remove the encrypted vault.",
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
      setDeleteArmed(false);
      clearPassphrase();
      setStatus({ kind: "neutral", message: "Local vault deleted from this browser." });
    } catch {
      setStatus({ kind: "error", message: "The local vault could not be deleted." });
    }
  }

  const unlocked = Boolean(session && !session.locked);

  return (
    <main id="main" className="vault-page">
      <section className="vault-hero">
        <p className="eyebrow">Holder vault · local browser boundary</p>
        <h1>Private evidence stays with the holder.</h1>
        <p>
          This is a real local encrypted vault. Zerant derives a wrapping key from your
          passphrase, wraps a random AES-256-GCM data key, and stores ciphertext in
          IndexedDB. Nothing here connects to a wallet or sends data to a server.
        </p>
      </section>

      <div className="vault-warning" role="note">
        <strong>Credential storage only.</strong>
        <span>
          Do not enter Zcash seed phrases, spending keys, wallet passwords, or other
          recovery secrets. Wallet authority remains outside the generic Zerant vault.
        </span>
      </div>

      <section className="vault-grid">
        <article className="vault-card">
          <div className="vault-card-heading">
            <div>
              <p className="eyebrow">01 · Access</p>
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
                Zerant never stores this passphrase. Losing it means the local ciphertext
                cannot be recovered by Zerant.
              </p>
              <Button onClick={envelope ? handleUnlock : handleCreate}>
                {envelope ? "Unlock local vault" : "Create encrypted vault"}
              </Button>
            </div>
          ) : (
            <div className="vault-actions">
              <Button variant="secondary" onClick={handleLock}>Lock now</Button>
              <span className="small muted">
                The data key is held only by this in-memory session reference.
              </span>
            </div>
          )}

          <p className={`vault-status ${status.kind}`} role="status" aria-live="polite">
            {status.message}
          </p>
        </article>

        <article className="vault-card">
          <div className="vault-card-heading">
            <div>
              <p className="eyebrow">02 · Encrypted records</p>
              <h2>{ids.length} local record{ids.length === 1 ? "" : "s"}</h2>
            </div>
            <span className="pill">IndexedDB only</span>
          </div>

          <div className="vault-record-list">
            {ids.length ? ids.map((id) => (
              <button
                type="button"
                className={selectedRecordId === id ? "vault-record selected" : "vault-record"}
                key={id}
                onClick={() => { setSelectedId(id); setPreview(null); }}
                aria-pressed={selectedRecordId === id}
              >
                <span className="mono">{id}</span>
                <span>opaque ID · ciphertext persisted</span>
              </button>
            )) : <p className="muted">No encrypted records yet.</p>}
          </div>

          <div className="vault-actions wrap">
            <Button disabled={!unlocked} onClick={handleAddSample}>Encrypt synthetic record</Button>
            <Button
              variant="secondary"
              disabled={!unlocked || !selectedRecordId}
              onClick={handleReadSample}
            >
              Decrypt synthetic record
            </Button>
            <Button
              variant="secondary"
              disabled={!unlocked || !selectedRecordId}
              onClick={handleRemoveSample}
            >
              Remove record
            </Button>
          </div>

          {preview ? (
            <pre className="vault-preview" aria-label="Decrypted synthetic local record">
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
          <p><strong>Implemented:</strong> AES-256-GCM, fresh 96-bit IVs, authenticated record IDs, PBKDF2-HMAC-SHA-256 with 600,000 iterations, random 128-bit salt, wrapped random data key, IndexedDB persistence.</p>
          <p><strong>Still exposed:</strong> a compromised browser/device or malicious same-origin script can read an unlocked vault. JavaScript cannot guarantee physical memory erasure.</p>
          <p><strong>Not mixed with wallet authority:</strong> Zcash spending keys, FROST shares and PCZT artifacts stay outside this browser vault.</p>
        </div>
      </section>

      <section className="vault-danger">
        <div>
          <p className="eyebrow">Local deletion</p>
          <h2>Remove this browser vault.</h2>
          <p className="muted">This removes the encrypted IndexedDB record from this browser. There is no server copy or recovery service.</p>
        </div>
        <Button variant="secondary" onClick={handleDeleteVault}>
          {deleteArmed ? "Delete local vault now" : "Delete local vault"}
        </Button>
      </section>
    </main>
  );
}
