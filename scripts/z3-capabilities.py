#!/usr/bin/env python3
"""Capture only method signatures; never capture wallet data, auth, or RPC errors."""
import base64
import json
import os
import urllib.request
import urllib.error

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError("redirect refused")

try:
    headers = {"Content-Type": "application/json"}
    if os.getenv("Z3_REGTEST_UNAUTHENTICATED") != "1":
        password = os.environ["Z3_REGTEST_RPC_ROUTER_PASSWORD"]
        auth = os.getenv("Z3_REGTEST_RPC_ROUTER_USER", "zebra") + ":" + password
        headers["Authorization"] = "Basic " + base64.b64encode(auth.encode()).decode()
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    req = urllib.request.Request("http://127.0.0.1:8181", json.dumps({"jsonrpc": "2.0", "id": 1, "method": "rpc.discover", "params": []}).encode(), headers)
    with opener.open(req, timeout=15) as response:
        raw = response.read(1_048_577)
    if len(raw) > 1_048_576:
        raise RuntimeError("response too large")
    envelope = json.loads(raw)
    if envelope.get("id") != 1 or envelope.get("error"):
        raise RuntimeError("RPC rejected")
    methods = envelope["result"]["methods"]
    if not isinstance(methods, list) or len(methods) > 1024:
        raise RuntimeError("method count")
    allowed = {"getblockchaininfo", "getwalletinfo", "generate", "generatetoaddress", "z_sendmany", "z_sendfromaccount", "z_viewtransaction", "z_shieldcoinbase", "z_getoperationstatus", "pczt_create", "pczt_inspect", "pczt_combine", "pczt_prove", "pczt_sign", "pczt_extract"}
    sanitized = [{"name": m["name"], "params": [{"name": p["name"], "required": bool(p.get("required", False))} for p in m.get("params", [])]} for m in methods if m["name"] in allowed]
    print(json.dumps({"source": "recorded live loopback rpc.discover; not current browser connectivity", "methods": sanitized}, indent=2))
except Exception:
    raise SystemExit("Capability capture failed; no live success claimed. Configure isolated Z3 regtest and explicit authentication mode.")
