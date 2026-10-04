import type { Metadata } from "next";
import { cookies } from "next/headers";
import {
  HolderRequests,
  type HolderVerificationRequest,
} from "@/components/holder-requests";
import { fetchZerantBackend } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Requests",
  description: "Review and approve private verification requests.",
};

export const dynamic = "force-dynamic";

export default async function RequestsPage() {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore.toString();

  let backendAvailable = false;
  let authenticated = false;
  let requests: HolderVerificationRequest[] = [];

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    backendAvailable = sessionResponse !== null;
    authenticated = Boolean(sessionResponse?.ok);

    if (authenticated) {
      const requestResponse = await fetchZerantBackend("/v1/holder/requests", cookieHeader);
      if (requestResponse?.ok) {
        requests = (await requestResponse.json()) as HolderVerificationRequest[];
      }
    }
  } catch {
    backendAvailable = false;
  }

  return (
    <HolderRequests
      authenticated={authenticated}
      backendAvailable={backendAvailable}
      initialRequests={requests}
    />
  );
}
