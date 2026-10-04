#!/usr/bin/env python3
"""Explicit synthetic regtest coinbase-shielding demo; no keys or raw results logged.

This is NOT a fully shielded z_sendmany integration or production invoice issuer.
The source transparent coinbase is revealed. Uses live OpenRPC method discovery.
"""
import base64
import json
import os
import sys
import time
import urllib.request

if sys.argv[1:] != ['--execute-regtest']:
    raise SystemExit('Usage: python3 scripts/z3-payment-demo.py --execute-regtest')

if not os.getenv('Z3_REGTEST_RPC_ROUTER_PASSWORD'):
    raise SystemExit('Set the isolated regtest router password in Z3_REGTEST_RPC_ROUTER_PASSWORD')

HEADERS = {"Content-Type": "application/json"}
HEADERS["Authorization"] = "Basic " + base64.b64encode(
    (os.getenv("Z3_REGTEST_RPC_ROUTER_USER", "zebra") + ":" + os.environ["Z3_REGTEST_RPC_ROUTER_PASSWORD"]).encode()
).decode()
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError("RPC redirects refused")
OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

LAST_METHOD = "none"
def call(method, params):
    global LAST_METHOD
    LAST_METHOD = method
    request = urllib.request.Request('http://127.0.0.1:8181', json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params}).encode(), HEADERS)
    with OPENER.open(request, timeout=30) as response:
        raw = response.read(1048577)
    if len(raw) > 1048576:
        raise RuntimeError('RPC size limit')
    envelope = json.loads(raw)
    if envelope.get('id') != 1 or envelope.get('error'):
        raise RuntimeError('RPC rejected ' + method)
    return envelope['result']

def operation(opid):
    for _ in range(90):
        values = call('z_getoperationstatus', [[opid]])
        if values and values[0]['status'] == 'success':
            result = values[0]['result']
            txids = result.get('txids', [result.get('txid')])
            if len(txids) != 1 or not isinstance(txids[0], str) or result.get('broadcast') is not True:
                raise RuntimeError('Unsupported transaction result')
            return txids[0]
        if values and values[0]['status'] in ('failed', 'cancelled'):
            raise RuntimeError('Local shielding operation failed')
        time.sleep(2)
    raise RuntimeError('Local shielding operation timeout')

try:
    schema = call('rpc.discover', [])
    names = {m['name'] for m in schema['methods']}
    required = {'generate', 'generatetoaddress', 'z_getnewaccount', 'z_getaddressforaccount', 'z_listunifiedreceivers', 'z_shieldcoinbase', 'z_getoperationstatus', 'z_viewtransaction'}
    if not required <= names:
        raise RuntimeError('Required live methods unavailable')
    # This RPC only succeeds on Regtest. Testnet's ambiguous "test" label is insufficient.
    call('generate', [1])
    account = call('z_getnewaccount', ['Zerant isolated synthetic shielding demo'])['account_uuid']
    funding_ua = call('z_getaddressforaccount', [account])['address']
    transparent = call('z_listunifiedreceivers', [funding_ua])['p2pkh']
    recipient = call('z_getaddressforaccount', [account, ['orchard']])['address']
    for batch in range(11):
        call('generatetoaddress', [10, transparent])
        if batch in (0, 5, 10):
            print('Synthetic regtest funding in progress', flush=True)
    # Allow wallet scan to catch up. Errors fail closed; no source fallback.
    time.sleep(5)
    opid = call('z_shieldcoinbase', [transparent, recipient, None, 1, None, 'AllowRevealedSenders'])['opid']
    txid = operation(opid)
    call('generatetoaddress', [3, transparent])
    for _ in range(30):
        view = call('z_viewtransaction', [txid])
        confirmations = view.get('confirmations', 0)
        matched = any(o.get('address') == recipient and type(o.get('valueZat')) is int and o['valueZat'] >= 100000000 and o.get('walletInternal') is False and o.get('pool') in ('sapling', 'orchard', 'ironwood') for o in view.get('outputs', []))
        if confirmations >= 3 and matched:
            print(json.dumps({'network': 'regtest', 'path': 'coinbase-shielding', 'confirmations': confirmations, 'recipient_and_minimum_amount_matched': True, 'fully_shielded_send_demonstrated': False}))
            break
        time.sleep(2)
    else:
        raise RuntimeError('Specific payment condition not observed')
except Exception:
    raise SystemExit('Regtest payment demo failed after ' + LAST_METHOD + '; no payment-condition success is claimed. Inspect the local stack privately.')
