import type { Metadata } from "next";
import { AccountSettings } from "@/components/account-settings";
import { loadAccountData } from "@/app/account/data";

export const metadata: Metadata = {
  title: "Account",
  description: "Export or manage your Zerant account data.",
};

export const dynamic = "force-dynamic";

export default async function AccountPage() {
  const data = await loadAccountData();
  return (
    <AccountSettings
      authenticated={data.authenticated}
      backendAvailable={data.backendAvailable}
      summary={data.summary}
      initialPasskeys={data.passkeys}
      initialSessions={data.sessions}
      initialZcashMethods={data.zcashMethods}
      activeSection="overview"
    />
  );
}
