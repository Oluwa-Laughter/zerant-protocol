import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  AccountSettings,
  type AccountSummary,
} from "@/components/account-settings";
import { fetchZerantBackend } from "@/lib/server-api";

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

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    backendAvailable = sessionResponse !== null;
    authenticated = Boolean(sessionResponse?.ok);

    if (authenticated) {
      const accountResponse = await fetchZerantBackend("/v1/account", cookieHeader);
      if (accountResponse?.ok) {
        summary = (await accountResponse.json()) as AccountSummary;
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
    />
  );
}
