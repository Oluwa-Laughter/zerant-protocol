import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  VerifierWorkspace,
  type TrustedIssuerOption,
  type VerificationRequestItem,
  type VerifierKeyView,
  type VerifierProfile,
} from "@/components/verifier-workspace";
import { fetchZerantBackend } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Verify",
  description: "Request narrow private proofs with Zerant.",
};

export const dynamic = "force-dynamic";

export default async function VerifierPage() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();

  let backendAvailable = false;
  let authenticated = false;
  let profile: VerifierProfile | null = null;
  let issuers: TrustedIssuerOption[] = [];
  let requests: VerificationRequestItem[] = [];
  let keys: VerifierKeyView[] = [];

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    backendAvailable = sessionResponse !== null;
    authenticated = Boolean(sessionResponse?.ok);

    if (authenticated) {
      const issuersResponse = await fetchZerantBackend("/v1/issuers", cookieHeader);
      if (issuersResponse?.ok) {
        issuers = (await issuersResponse.json()) as TrustedIssuerOption[];
      }

      const profileResponse = await fetchZerantBackend("/v1/verifier", cookieHeader);
      if (profileResponse?.ok) {
        profile = (await profileResponse.json()) as VerifierProfile;
        const [requestsResponse, keysResponse] = await Promise.all([
          fetchZerantBackend("/v1/verifier/requests", cookieHeader),
          fetchZerantBackend("/v1/verifier/keys", cookieHeader),
        ]);
        if (requestsResponse?.ok) {
          requests = (await requestsResponse.json()) as VerificationRequestItem[];
        }
        if (keysResponse?.ok) {
          keys = (await keysResponse.json()) as VerifierKeyView[];
        }
      }
    }
  } catch {
    backendAvailable = false;
  }

  return (
    <VerifierWorkspace
      authenticated={authenticated}
      backendAvailable={backendAvailable}
      initialProfile={profile}
      issuers={issuers}
      initialRequests={requests}
      initialKeys={keys}
    />
  );
}
