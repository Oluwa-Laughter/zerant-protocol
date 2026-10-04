import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  ProductWorkspace,
  type WorkspaceSession,
  type WorkspaceZcash,
  type WorkspaceZcashNetwork,
} from "@/components/product-workspace";
import { fetchZerantBackend } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Workspace",
  description: "Zerant holder, verifier and Zcash integration workspace.",
};

export const dynamic = "force-dynamic";

export default async function AppPage() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();

  let session: WorkspaceSession = null;
  let zcash: WorkspaceZcash = null;
  let zcashNetwork: WorkspaceZcashNetwork = null;

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    if (sessionResponse?.ok) {
      session = (await sessionResponse.json()) as NonNullable<WorkspaceSession>;
      const [zcashResponse, networkResponse] = await Promise.all([
        fetchZerantBackend("/v1/zcash/status", cookieHeader),
        fetchZerantBackend("/v1/zcash/network/readiness", cookieHeader),
      ]);
      if (zcashResponse?.ok) {
        zcash = (await zcashResponse.json()) as NonNullable<WorkspaceZcash>;
      }
      if (networkResponse?.ok) {
        zcashNetwork =
          (await networkResponse.json()) as NonNullable<WorkspaceZcashNetwork>;
      }
    }
  } catch {
    session = null;
    zcash = null;
    zcashNetwork = null;
  }

  return (
    <ProductWorkspace session={session} zcash={zcash} zcashNetwork={zcashNetwork} />
  );
}
