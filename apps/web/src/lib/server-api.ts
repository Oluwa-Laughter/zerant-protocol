import "server-only";

const MAX_PROXY_BODY = 300_000;

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

  return new Response(await upstream.arrayBuffer(), {
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
