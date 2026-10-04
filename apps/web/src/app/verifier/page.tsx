import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  VerifierWorkspace,
  type TrustedIssuerOption,
  type VerificationRequestItem,
  type VerifierApiKeyView,
  type VerifierKeyView,
  type VerifierProfile,
  type VerifierWebhookView,
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
  let apiKeys: VerifierApiKeyView[] = [];
  let webhooks: VerifierWebhookView[] = [];

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
        const [requestsResponse, keysResponse, apiKeysResponse, webhooksResponse] =
          await Promise.all([
          fetchZerantBackend("/v1/verifier/requests", cookieHeader),
          fetchZerantBackend("/v1/verifier/keys", cookieHeader),
            fetchZerantBackend("/v1/verifier/api-keys", cookieHeader),
            fetchZerantBackend("/v1/verifier/webhooks", cookieHeader),
          ]);
        if (requestsResponse?.ok) {
          requests = (await requestsResponse.json()) as VerificationRequestItem[];
        }
        if (keysResponse?.ok) {
          keys = (await keysResponse.json()) as VerifierKeyView[];
        }
        if (apiKeysResponse?.ok) {
          apiKeys = (await apiKeysResponse.json()) as VerifierApiKeyView[];
        }
        if (webhooksResponse?.ok) {
          webhooks = (await webhooksResponse.json()) as VerifierWebhookView[];
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
      initialApiKeys={apiKeys}
      initialWebhooks={webhooks}
    />
  );
}
