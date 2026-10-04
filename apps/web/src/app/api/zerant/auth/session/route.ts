import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  return proxyToZerant(request, "/v1/auth/session", { method: "GET" });
}
