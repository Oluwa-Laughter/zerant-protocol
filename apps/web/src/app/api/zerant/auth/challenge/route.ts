import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  const url = new URL(request.url);
  const scopes = url.searchParams.get("scopes");
  const query = scopes ? "?scopes=" + encodeURIComponent(scopes) : "";
  return proxyToZerant(request, "/v1/auth/zecauth/challenge" + query, { method: "GET" });
}
