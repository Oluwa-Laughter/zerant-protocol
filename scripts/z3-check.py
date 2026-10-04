#!/usr/bin/env python3
"""Read-only local Z3 regtest probe. Never prints raw wallet results or credentials."""
import base64
import json
import os
import urllib.request
import urllib.error

URL = "http://127.0.0.1:8181"
ALLOWED_METHODS = {"rpc.discover", "getblockchaininfo", "getwalletinfo"}

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise urllib.error.HTTPError(req.full_url, code, "redirect refused", headers, fp)

opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

def credentials():
    if os.getenv("Z3_REGTEST_UNAUTHENTICATED") == "1":
        return None
    user = os.getenv("Z3_REGTEST_RPC_ROUTER_USER", "zebra")
    password = os.getenv("Z3_REGTEST_RPC_ROUTER_PASSWORD")
    if not password:
        raise RuntimeError("Z3_REGTEST_RPC_ROUTER_PASSWORD is required")
    return user, password

def call(method):
    if method not in ALLOWED_METHODS:
        raise RuntimeError("RPC method not allowlisted")
    credential = credentials()
    headers = {"Content-Type": "application/json"}
    if credential:
        user, password = credential
        headers["Authorization"] = "Basic " + base64.b64encode((user + ":" + password).encode()).decode()
    request = urllib.request.Request(
        URL,
        json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": []}).encode(),
        headers,
        method="POST",
    )
    with opener.open(request, timeout=15) as response:
        raw = response.read(1_048_577)
    if len(raw) > 1_048_576:
        raise RuntimeError("response too large")
    value = json.loads(raw)
    if value.get("id") != 1 or value.get("error") or "result" not in value:
        raise RuntimeError("RPC rejected " + method)
    return value["result"]

try:
    schema = call("rpc.discover")
    names = {m["name"] for m in schema["methods"]}
    assert {"getblockchaininfo", "getwalletinfo"} <= names
    chain = call("getblockchaininfo")
    assert chain["chain"] in ("test", "regtest")
    wallet = call("getwalletinfo")
    assert isinstance(wallet["walletversion"], int)
    print(json.dumps({
        "mode": "regtest-local",
        "blocks": chain["blocks"],
        "wallet_rpc_reachable": True,
        "payment_exercised": False,
        "pczt_complete": all(m in names for m in ("pczt_create", "pczt_inspect", "pczt_prove", "pczt_sign", "pczt_combine", "pczt_extract")),
        "sendfromaccount_advertised": "z_sendfromaccount" in names,
        "receipt_verification": "z_viewtransaction" in names,
        "methods": sorted(names & {"getblockchaininfo", "getwalletinfo", "z_sendmany", "z_viewtransaction", "z_sendfromaccount", "pczt_create", "pczt_inspect", "pczt_prove", "pczt_sign", "pczt_combine", "pczt_extract"}),
    }))
except Exception:
    raise SystemExit(
        "Regtest probe failed. Start the official local Z3 regtest stack and set "
        "Z3_REGTEST_RPC_ROUTER_PASSWORD; no live success is claimed."
    )
