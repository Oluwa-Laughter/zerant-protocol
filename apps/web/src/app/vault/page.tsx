import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  ServerCredentialVault,
  type ServerVaultCredential,
  type ServerVaultSession,
} from "@/components/server-credential-vault";
import { fetchZerantBackend } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Credential vault",
  description: "Server-backed encrypted credential storage authenticated with Zcash ZecAuth.",
};

export const dynamic = "force-dynamic";

export default async function VaultPage() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();

  let backendAvailable = false;
  let session: ServerVaultSession | null = null;
  let credentials: ServerVaultCredential[] = [];

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    backendAvailable = sessionResponse !== null;
    if (sessionResponse?.ok) {
      session = (await sessionResponse.json()) as ServerVaultSession;
      const credentialResponse = await fetchZerantBackend("/v1/credentials", cookieHeader);
      if (credentialResponse?.ok) {
        credentials = (await credentialResponse.json()) as ServerVaultCredential[];
      }
    }
  } catch {
    backendAvailable = false;
  }

  return (
    <ServerCredentialVault
      initialSession={session}
      initialCredentials={credentials}
      backendAvailable={backendAvailable}
    />
  );
}
