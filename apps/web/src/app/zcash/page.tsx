import type { Metadata } from "next";
import { cookies } from "next/headers";
import { ZcashWorkspace } from "@/components/zcash-workspace";
import type { WorkspaceSession, WorkspaceZcash, WorkspaceZcashNetwork } from "@/components/product-workspace";
import { fetchZerantBackend } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Zcash",
  description: "Private Zcash actions, payment review, and network readiness inside Zerant.",
};

export const dynamic = "force-dynamic";

export default async function ZcashPage() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();

  let session: WorkspaceSession = null;
  let zcash: WorkspaceZcash = null;
  let network: WorkspaceZcashNetwork = null;

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    if (sessionResponse?.ok) {
      session = (await sessionResponse.json()) as NonNullable<WorkspaceSession>;
      const [statusResponse, readinessResponse] = await Promise.all([
        fetchZerantBackend("/v1/zcash/status", cookieHeader),
        fetchZerantBackend("/v1/zcash/network/readiness", cookieHeader),
      ]);
      if (statusResponse?.ok) zcash = (await statusResponse.json()) as NonNullable<WorkspaceZcash>;
      if (readinessResponse?.ok) network = (await readinessResponse.json()) as NonNullable<WorkspaceZcashNetwork>;
    }
  } catch {
    session = null;
    zcash = null;
    network = null;
  }

  return <ZcashWorkspace session={session} zcash={zcash} network={network} />;
}
