import { cookies } from "next/headers";
import { fetchZerantBackend } from "@/lib/server-api";
import type { AccountSummary } from "@/components/account-settings";
import type { PasskeyView } from "@/components/passkey-manager";
import type { AccountSessionView } from "@/components/session-manager";
import type { LinkedZcashMethod } from "@/components/zcash-sign-in-manager";

export async function loadAccountData() {
  const cookieHeader = (await cookies()).toString();
  let backendAvailable = false;
  let authenticated = false;
  let summary: AccountSummary | null = null;
  let passkeys: PasskeyView[] = [];
  let sessions: AccountSessionView[] = [];
  let zcashMethods: LinkedZcashMethod[] = [];
  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    backendAvailable = sessionResponse !== null;
    authenticated = Boolean(sessionResponse?.ok);
    if (authenticated) {
      const accountResponse = await fetchZerantBackend("/v1/account", cookieHeader);
      if (accountResponse?.ok) summary = await accountResponse.json() as AccountSummary;
      const [passkeyResponse, sessionListResponse, zcashResponse] = await Promise.all([
        fetchZerantBackend("/v1/account/passkeys", cookieHeader),
        fetchZerantBackend("/v1/account/sessions", cookieHeader),
        fetchZerantBackend("/v1/account/zcash/methods", cookieHeader),
      ]);
      if (passkeyResponse?.ok) passkeys = await passkeyResponse.json() as PasskeyView[];
      if (sessionListResponse?.ok) sessions = await sessionListResponse.json() as AccountSessionView[];
      if (zcashResponse?.ok) zcashMethods = await zcashResponse.json() as LinkedZcashMethod[];
    }
  } catch {
    backendAvailable = false;
  }
  return { backendAvailable, authenticated, summary, passkeys, sessions, zcashMethods };
}
