import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  IssuerWorkspace,
  type IssuedCredential,
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

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    backendAvailable = sessionResponse !== null;
    authenticated = Boolean(sessionResponse?.ok);

    if (authenticated) {
      const profileResponse = await fetchZerantBackend("/v1/issuer", cookieHeader);
      if (profileResponse?.ok) {
        profile = (await profileResponse.json()) as IssuerProfile;
        const issuedResponse = await fetchZerantBackend("/v1/issuer/credentials", cookieHeader);
        if (issuedResponse?.ok) {
          issued = (await issuedResponse.json()) as IssuedCredential[];
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
    />
  );
}
