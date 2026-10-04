import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  return proxyToZerant(request, "/v1/verifier/webhooks", { method: "GET" });
}

export async function POST(request: Request) {
  return proxyToZerant(request, "/v1/verifier/webhooks", { method: "POST" });
}
