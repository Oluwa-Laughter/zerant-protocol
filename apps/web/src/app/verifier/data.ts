import { cookies } from "next/headers";
import { fetchZerantBackend } from "@/lib/server-api";
import type {
  TrustedIssuerOption,
  VerificationRequestItem,
  VerifierApiKeyView,
  VerifierKeyView,
  VerifierProfile,
  VerifierWebhookView,
} from "@/components/verifier-workspace";

export type VerifierWorkspaceData = {
  authenticated: boolean;
  backendAvailable: boolean;
  profile: VerifierProfile | null;
  issuers: TrustedIssuerOption[];
  requests: VerificationRequestItem[];
  keys: VerifierKeyView[];
  apiKeys: VerifierApiKeyView[];
  webhooks: VerifierWebhookView[];
};

export async function loadVerifierWorkspaceData(): Promise<VerifierWorkspaceData> {
  const cookieHeader = (await cookies()).toString();
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
      if (issuersResponse?.ok) issuers = await issuersResponse.json() as TrustedIssuerOption[];
      const profileResponse = await fetchZerantBackend("/v1/verifier", cookieHeader);
      if (profileResponse?.ok) {
        profile = await profileResponse.json() as VerifierProfile;
        const [requestsResponse, keysResponse, apiKeysResponse, webhooksResponse] = await Promise.all([
          fetchZerantBackend("/v1/verifier/requests", cookieHeader),
          fetchZerantBackend("/v1/verifier/keys", cookieHeader),
          fetchZerantBackend("/v1/verifier/api-keys", cookieHeader),
          fetchZerantBackend("/v1/verifier/webhooks", cookieHeader),
        ]);
        if (requestsResponse?.ok) requests = await requestsResponse.json() as VerificationRequestItem[];
        if (keysResponse?.ok) keys = await keysResponse.json() as VerifierKeyView[];
        if (apiKeysResponse?.ok) apiKeys = await apiKeysResponse.json() as VerifierApiKeyView[];
        if (webhooksResponse?.ok) webhooks = await webhooksResponse.json() as VerifierWebhookView[];
      }
    }
  } catch {
    backendAvailable = false;
  }

  return { authenticated, backendAvailable, profile, issuers, requests, keys, apiKeys, webhooks };
}
