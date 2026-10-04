import { proxyToZerant } from "@/lib/server-api";
export const dynamic = "force-dynamic";
export async function POST(request: Request) {
  return proxyToZerant(request, "/v1/issuer/ownership/transfer", { method: "POST" });
}
