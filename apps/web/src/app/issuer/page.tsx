import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  IssuerWorkspace,
  type CredentialSchema,
  type IssuedCredential,
  type IssuerKeyView,
  type IssuerProfile,
} from "@/components/issuer-workspace";
import { fetchZerantBackend } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Issuers",
  description: "Issue private, portable credentials with Zerant.",
};

export const dynamic = "force-dynamic";

export default async function IssuerPage() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();

  let backendAvailable = false;
  let authenticated = false;
  let profile: IssuerProfile | null = null;
  let issued: IssuedCredential[] = [];
  let schemas: CredentialSchema[] = [];
  let keys: IssuerKeyView[] = [];

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    backendAvailable = sessionResponse !== null;
    authenticated = Boolean(sessionResponse?.ok);

    if (authenticated) {
      const profileResponse = await fetchZerantBackend("/v1/issuer", cookieHeader);
      if (profileResponse?.ok) {
        profile = (await profileResponse.json()) as IssuerProfile;

        const [issuedResponse, schemasResponse, keysResponse] = await Promise.all([
          fetchZerantBackend("/v1/issuer/credentials", cookieHeader),
          fetchZerantBackend("/v1/issuer/schemas", cookieHeader),
          fetchZerantBackend("/v1/issuer/keys", cookieHeader),
        ]);

        if (issuedResponse?.ok) {
          issued = (await issuedResponse.json()) as IssuedCredential[];
        }
        if (schemasResponse?.ok) {
          schemas = (await schemasResponse.json()) as CredentialSchema[];
        }
        if (keysResponse?.ok) {
          keys = (await keysResponse.json()) as IssuerKeyView[];
        }
      }
    }
  } catch {
    backendAvailable = false;
  }

  return (
    <IssuerWorkspace
      authenticated={authenticated}
      backendAvailable={backendAvailable}
      initialProfile={profile}
      initialIssued={issued}
      initialSchemas={schemas}
      initialKeys={keys}
    />
  );
}
