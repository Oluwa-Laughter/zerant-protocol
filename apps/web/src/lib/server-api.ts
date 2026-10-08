import "server-only";
import type { OperationalHealthSnapshot } from "@/lib/operational-health";

const MAX_PROXY_BODY = 300_000;
const NULL_BODY_STATUSES = new Set([204, 205, 304]);

async function proxyResponseBody(upstream: Response, method: string): Promise<ArrayBuffer | null> {
  if (method === "HEAD" || NULL_BODY_STATUSES.has(upstream.status)) return null;
  return upstream.arrayBuffer();
}

function backendOrigin(): string {
  const raw = process.env.ZERANT_API_ORIGIN;
  if (!raw) throw new Error("ZERANT_API_ORIGIN is not configured");
  const url = new URL(raw);
  if (process.env.NODE_ENV === "production" && url.protocol !== "https:") {
    throw new Error("ZERANT_API_ORIGIN must use HTTPS in production");
  }
  return url.origin;
}

export async function proxyToZerant(
  request: Request,
  path: string,
  options?: { method?: string },
): Promise<Response> {
  let body: ArrayBuffer | undefined;
  const method = options?.method ?? request.method;
  if (!["GET", "HEAD"].includes(method)) {
    body = await request.arrayBuffer();
    if (body.byteLength > MAX_PROXY_BODY) {
      return Response.json({ error: "request too large" }, { status: 413 });
    }
  }

  const headers = new Headers();
  const cookie = request.headers.get("cookie");
  if (cookie) headers.set("cookie", cookie);
  const contentType = request.headers.get("content-type");
  if (contentType) headers.set("content-type", contentType);

  let upstream: Response;
  try {
    upstream = await fetch(new URL(path, backendOrigin()), {
      method,
      headers,
      body,
      redirect: "manual",
      cache: "no-store",
    });
  } catch {
    return Response.json({ error: "Zerant API unavailable" }, { status: 503 });
  }

  const responseHeaders = new Headers();
  const upstreamType = upstream.headers.get("content-type");
  if (upstreamType) responseHeaders.set("content-type", upstreamType);
  responseHeaders.set("cache-control", "no-store");

  const getter = (upstream.headers as Headers & { getSetCookie?: () => string[] }).getSetCookie;
  const cookies = getter?.call(upstream.headers) ?? [];
  if (cookies.length) {
    for (const value of cookies) responseHeaders.append("set-cookie", value);
  } else {
    const single = upstream.headers.get("set-cookie");
    if (single) responseHeaders.append("set-cookie", single);
  }

  return new Response(await proxyResponseBody(upstream, method), {
    status: upstream.status,
    headers: responseHeaders,
  });
}

export async function fetchZerantBackend(
  path: string,
  cookieHeader?: string,
): Promise<Response | null> {
  const headers = new Headers();
  if (cookieHeader) headers.set("cookie", cookieHeader);
  try {
    return await fetch(new URL(path, backendOrigin()), {
      method: "GET",
      headers,
      redirect: "manual",
      cache: "no-store",
    });
  } catch {
    return null;
  }
}


export async function proxyPublicToZerant(
  path: string,
  options?: { cacheControl?: string },
): Promise<Response> {
  let upstream: Response;
  try {
    upstream = await fetch(new URL(path, backendOrigin()), {
      method: "GET",
      redirect: "manual",
      cache: "no-store",
    });
  } catch {
    return Response.json({ error: "Zerant API unavailable" }, { status: 503 });
  }

  const responseHeaders = new Headers();
  const contentType = upstream.headers.get("content-type");
  if (contentType) responseHeaders.set("content-type", contentType);
  const cacheControl = upstream.headers.get("cache-control");
  responseHeaders.set(
    "cache-control",
    options?.cacheControl ?? cacheControl ?? "public, max-age=60, stale-while-revalidate=60",
  );

  return new Response(await proxyResponseBody(upstream, "GET"), {
    status: upstream.status,
    headers: responseHeaders,
  });
}

export async function fetchZerantPublic(path: string): Promise<Response | null> {
  try {
    return await fetch(new URL(path, backendOrigin()), {
      method: "GET",
      redirect: "manual",
      cache: "no-store",
    });
  } catch {
    return null;
  }
}

export async function proxyIntegrationToZerant(
  request: Request,
  path: string,
  options?: { method?: string },
): Promise<Response> {
  const authorization = request.headers.get("authorization");
  if (!authorization?.startsWith("Bearer ")) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }

  const method = options?.method ?? request.method;
  let body: ArrayBuffer | undefined;
  if (!["GET", "HEAD"].includes(method)) {
    body = await request.arrayBuffer();
    if (body.byteLength > MAX_PROXY_BODY) {
      return Response.json({ error: "request too large" }, { status: 413 });
    }
  }

  const headers = new Headers({ authorization });
  const contentType = request.headers.get("content-type");
  if (contentType) headers.set("content-type", contentType);

  let upstream: Response;
  try {
    upstream = await fetch(new URL(path, backendOrigin()), {
      method,
      headers,
      body,
      redirect: "manual",
      cache: "no-store",
    });
  } catch {
    return Response.json({ error: "Zerant API unavailable" }, { status: 503 });
  }

  const responseHeaders = new Headers({ "cache-control": "no-store" });
  const upstreamType = upstream.headers.get("content-type");
  if (upstreamType) responseHeaders.set("content-type", upstreamType);

  return new Response(await proxyResponseBody(upstream, method), {
    status: upstream.status,
    headers: responseHeaders,
  });
}

export type WebhookDispatchSummary = {
  claimed: number;
  delivered: number;
  retried: number;
  dead: number;
  pending: number;
  finalized: boolean;
};

export async function dispatchWebhookRequestToZerant(
  requestId: string,
): Promise<WebhookDispatchSummary> {
  let upstream: Response;
  try {
    upstream = await fetch(
      new URL(
        "/v1/internal/webhooks/requests/" + encodeURIComponent(requestId) + "/dispatch",
        backendOrigin(),
      ),
      {
        method: "POST",
        redirect: "manual",
        cache: "no-store",
      },
    );
  } catch {
    throw new Error("Zerant webhook dispatcher unavailable");
  }

  if (upstream.status === 404) {
    return {
      claimed: 0,
      delivered: 0,
      retried: 0,
      dead: 0,
      pending: 0,
      finalized: true,
    };
  }
  if (!upstream.ok) {
    throw new Error("Zerant webhook dispatcher failed");
  }

  return (await upstream.json()) as WebhookDispatchSummary;
}

export type RetentionMaintenanceSummary = {
  skipped: boolean;
  expired_requests: number;
  sessions: number;
  zecauth_challenges: number;
  passkey_challenges: number;
  rate_limits: number;
  webhook_deliveries: number;
  proof_material: number;
  expired_payments: number;
  deleted_expired_payments: number;
  expired_invoices: number;
  deleted_expired_invoices: number;
  expired_payout_destinations: number;
  deleted_payout_destinations: number;
};

export async function runRetentionMaintenanceToZerant(): Promise<RetentionMaintenanceSummary> {
  let upstream: Response;
  try {
    upstream = await fetch(new URL("/v1/internal/maintenance/retention", backendOrigin()), {
      method: "POST",
      redirect: "manual",
      cache: "no-store",
    });
  } catch {
    throw new Error("Zerant retention maintenance unavailable");
  }

  if (!upstream.ok) {
    throw new Error("Zerant retention maintenance failed");
  }
  return (await upstream.json()) as RetentionMaintenanceSummary;
}

export async function fetchOperationalHealthToZerant(): Promise<OperationalHealthSnapshot> {
  let upstream: Response;
  try {
    upstream = await fetch(new URL("/v1/internal/ops/health", backendOrigin()), {
      method: "GET",
      redirect: "manual",
      cache: "no-store",
    });
  } catch {
    throw new Error("Zerant operational health unavailable");
  }

  if (!upstream.ok) {
    throw new Error("Zerant operational health failed");
  }
  return (await upstream.json()) as OperationalHealthSnapshot;
}
