import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export function GET(request: Request) {
  return proxyToZerant(request, "/v1/zcash/payout-destinations");
}

export function POST(request: Request) {
  return proxyToZerant(request, "/v1/zcash/payout-destinations", { method: "POST" });
}
