import { parseVaultEnvelope, type VaultEnvelopeV1, VaultError } from "./vault";

const DB_NAME = "zerant-holder-vault";
const DB_VERSION = 1;
const STORE = "vaults";
const DEFAULT_KEY = "default";

function indexedDb(): IDBFactory {
  if (typeof indexedDB === "undefined") throw new VaultError("IndexedDB is unavailable");
  return indexedDB;
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDb().open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE)) db.createObjectStore(STORE);
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(new VaultError("Vault storage is unavailable"));
    request.onblocked = () => reject(new VaultError("Vault storage upgrade is blocked"));
  });
}

export async function loadVault(): Promise<VaultEnvelopeV1 | null> {
  const db = await openDatabase();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(STORE, "readonly");
      const request = tx.objectStore(STORE).get(DEFAULT_KEY);
      request.onsuccess = () => {
        if (request.result === undefined) resolve(null);
        else {
          try {
            resolve(parseVaultEnvelope(request.result));
          } catch {
            reject(new VaultError("Stored vault data is invalid"));
          }
        }
      };
      request.onerror = () => reject(new VaultError("Vault could not be loaded"));
    });
  } finally {
    db.close();
  }
}

export async function saveVault(envelope: VaultEnvelopeV1): Promise<void> {
  const validated = parseVaultEnvelope(envelope);
  const db = await openDatabase();
  try {
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE, "readwrite");
      tx.objectStore(STORE).put(validated, DEFAULT_KEY);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(new VaultError("Vault could not be saved"));
      tx.onabort = () => reject(new VaultError("Vault could not be saved"));
    });
  } finally {
    db.close();
  }
}

export async function deleteVault(): Promise<void> {
  const db = await openDatabase();
  try {
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE, "readwrite");
      tx.objectStore(STORE).delete(DEFAULT_KEY);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(new VaultError("Vault could not be deleted"));
      tx.onabort = () => reject(new VaultError("Vault could not be deleted"));
    });
  } finally {
    db.close();
  }
}
