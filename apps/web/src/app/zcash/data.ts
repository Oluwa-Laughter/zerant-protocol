import { cookies } from "next/headers";
import { fetchZerantBackend } from "@/lib/server-api";
import type { WorkspaceSession, WorkspaceZcash, WorkspaceZcashNetwork } from "@/components/product-workspace";

export async function loadZcashWorkspaceData(): Promise<{
  session: WorkspaceSession;
  zcash: WorkspaceZcash;
  network: WorkspaceZcashNetwork;
}> {
  const cookieHeader = (await cookies()).toString();
  let session: WorkspaceSession = null;
  let zcash: WorkspaceZcash = null;
  let network: WorkspaceZcashNetwork = null;
  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    if (sessionResponse?.ok) {
      session = await sessionResponse.json() as NonNullable<WorkspaceSession>;
      const [statusResponse, readinessResponse] = await Promise.all([
        fetchZerantBackend("/v1/zcash/status", cookieHeader),
        fetchZerantBackend("/v1/zcash/network/readiness", cookieHeader),
      ]);
      if (statusResponse?.ok) zcash = await statusResponse.json() as NonNullable<WorkspaceZcash>;
      if (readinessResponse?.ok) network = await readinessResponse.json() as NonNullable<WorkspaceZcashNetwork>;
    }
  } catch {
    session = null;
    zcash = null;
    network = null;
  }
  return { session, zcash, network };
}
