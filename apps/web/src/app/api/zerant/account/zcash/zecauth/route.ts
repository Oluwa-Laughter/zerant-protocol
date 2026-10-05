import { proxyToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function DELETE(request: Request) {
  return proxyToZerant(request, "/v1/account/zcash/zecauth", { method: "DELETE" });
}
