import { proxyIntegrationToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function POST(request: Request) {
  return proxyIntegrationToZerant(
    request,
    "/v1/integrations/verifier/requests",
    { method: "POST" },
  );
}
