import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  ProductWorkspace,
  type WorkspaceSession,
  type WorkspaceZcash,
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

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    if (sessionResponse?.ok) {
      session = (await sessionResponse.json()) as NonNullable<WorkspaceSession>;
      const zcashResponse = await fetchZerantBackend("/v1/zcash/status", cookieHeader);
      if (zcashResponse?.ok) {
        zcash = (await zcashResponse.json()) as NonNullable<WorkspaceZcash>;
      }
    }
  } catch {
    session = null;
    zcash = null;
  }

  return <ProductWorkspace session={session} zcash={zcash} />;
}
