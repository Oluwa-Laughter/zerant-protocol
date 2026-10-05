import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  ProductWorkspace,
  type WorkspaceSession,
  type WorkspaceZcash,
  type WorkspaceZcashNetwork,
  type WorkspaceAttention,
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
  let attention: WorkspaceAttention | null = null;

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    if (sessionResponse?.ok) {
      session = (await sessionResponse.json()) as NonNullable<WorkspaceSession>;
      const [zcashResponse, networkResponse, credentialResponse, requestResponse, paymentResponse] = await Promise.all([
        fetchZerantBackend("/v1/zcash/status", cookieHeader),
        fetchZerantBackend("/v1/zcash/network/readiness", cookieHeader),
        fetchZerantBackend("/v1/credentials", cookieHeader),
        fetchZerantBackend("/v1/holder/requests", cookieHeader),
        fetchZerantBackend("/v1/zcash/payments", cookieHeader),
      ]);
      if (zcashResponse?.ok) {
        zcash = (await zcashResponse.json()) as NonNullable<WorkspaceZcash>;
      }
      if (networkResponse?.ok) {
        zcashNetwork =
          (await networkResponse.json()) as NonNullable<WorkspaceZcashNetwork>;
      }

      const credentials = credentialResponse?.ok
        ? await credentialResponse.json() as Array<{ revoked: boolean }>
        : [];
      const requests = requestResponse?.ok
        ? await requestResponse.json() as unknown[]
        : [];
      const paymentPage = paymentResponse?.ok
        ? await paymentResponse.json() as { items?: Array<{ state: string }> }
        : { items: [] };
      const paymentItems = Array.isArray(paymentPage.items) ? paymentPage.items : [];
      attention = {
        activeCredentials: credentials.filter((item) => !item.revoked).length,
        pendingRequests: requests.length,
        preparedPayments: paymentItems.filter((item) => item.state === "prepared").length,
        submittedPayments: paymentItems.filter((item) => item.state === "submitted").length,
      };
    }
  } catch {
    session = null;
    zcash = null;
    zcashNetwork = null;
    attention = null;
  }

  return (
    <ProductWorkspace session={session} zcash={zcash} zcashNetwork={zcashNetwork} attention={attention} />
  );
}
