import { proxyToZerant } from "@/lib/server-api";

export async function POST(request: Request) {
  return proxyToZerant(request, "/v1/auth/passkey/discoverable/finish", { method: "POST" });
}
