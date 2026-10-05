import { isAuthorizedCronRequest } from "@/lib/cron-auth";
import { operationalProbeStatus } from "@/lib/operational-health";
import { fetchOperationalHealthToZerant } from "@/lib/server-api";

export const dynamic = "force-dynamic";

export async function GET(request: Request) {
  const secret = process.env.CRON_SECRET;
  if (!secret) {
    return Response.json({ error: "operational health unavailable" }, { status: 503 });
  }
  if (!isAuthorizedCronRequest(request.headers.get("authorization"), secret)) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }

  try {
    const snapshot = await fetchOperationalHealthToZerant();
    return Response.json(snapshot, {
      status: operationalProbeStatus(snapshot),
      headers: { "cache-control": "no-store" },
    });
  } catch {
    return Response.json({ error: "operational health failed" }, { status: 503 });
  }
}
