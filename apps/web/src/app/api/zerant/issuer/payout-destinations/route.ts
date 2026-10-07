import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export function GET(request: Request) {
  return proxyToZerant(request, "/v1/issuer/payout-destinations");
}
