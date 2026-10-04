import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  AccountSettings,
  type AccountSummary,
} from "@/components/account-settings";
import { fetchZerantBackend } from "@/lib/server-api";
import type { PasskeyView } from "@/components/passkey-manager";
import type { AccountSessionView } from "@/components/session-manager";
import type { LinkedZcashMethod } from "@/components/zcash-sign-in-manager";

export const metadata: Metadata = {
  title: "Account",
  description: "Export or manage your Zerant account data.",
};

export const dynamic = "force-dynamic";

export default async function AccountPage() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();

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
      if (accountResponse?.ok) {
        summary = (await accountResponse.json()) as AccountSummary;
      }
      const passkeyResponse = await fetchZerantBackend("/v1/account/passkeys", cookieHeader);
      if (passkeyResponse?.ok) {
        passkeys = (await passkeyResponse.json()) as PasskeyView[];
      }
      const sessionListResponse = await fetchZerantBackend("/v1/account/sessions", cookieHeader);
      if (sessionListResponse?.ok) {
        sessions = (await sessionListResponse.json()) as AccountSessionView[];
      }
      const zcashResponse = await fetchZerantBackend("/v1/account/zcash/methods", cookieHeader);
      if (zcashResponse?.ok) {
        zcashMethods = (await zcashResponse.json()) as LinkedZcashMethod[];
      }
    }
  } catch {
    backendAvailable = false;
  }

  return (
    <AccountSettings
      authenticated={authenticated}
      backendAvailable={backendAvailable}
      summary={summary}
      initialPasskeys={passkeys}
      initialSessions={sessions}
      initialZcashMethods={zcashMethods}
    />
  );
}
