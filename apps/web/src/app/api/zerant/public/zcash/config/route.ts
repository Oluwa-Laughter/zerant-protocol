import { proxyPublicToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET() {
  return proxyPublicToZerant("/v1/public/zcash/config");
}
