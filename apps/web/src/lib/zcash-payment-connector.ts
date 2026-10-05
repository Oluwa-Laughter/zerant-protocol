import type { ZcashConnector } from "./zcash-connectors";
import { zatoshiToZec, type ShieldedPayment } from "./zcash-wallet";

export type ReviewedPayment = {
  index: number; recipient: string; amount_zat: number | null;
  memo_present: boolean; label: string | null; message: string | null;
  other_param_names: string[];
};
export type ReviewedRequest = {
  canonical_uri: string; payment_count: number; payments: ReviewedPayment[];
};
export type DirectPaymentMode = "shielded" | "transparent" | null;

/** Direct calls must preserve every field of one exact, simple payment. */
export function directPaymentMode(request: ReviewedRequest, connector: ZcashConnector | null): DirectPaymentMode {
  if (!connector || request.payment_count !== 1 || request.payments.length !== 1) return null;
  const payment = request.payments[0];
  if (!payment || payment.index !== 0 || payment.amount_zat === null ||
      !Number.isSafeInteger(payment.amount_zat) || payment.amount_zat <= 0 ||
      payment.memo_present || payment.label !== null || payment.message !== null ||
      payment.other_param_names.length > 0) return null;
  if (connector.capabilities.has("shieldedPayment") && connector.sendShieldedPayment &&
      /^(u|zs|utest|ztestsapling)/.test(payment.recipient)) return "shielded";
  if (connector.capabilities.has("transparentPayment") && connector.sendTransparentPayment &&
      /^(t1|t3|tm|t2)/.test(payment.recipient)) return "transparent";
  return null;
}

export function paymentAction(connector: ZcashConnector, request: ReviewedRequest, options: { allowTransparent?: boolean } = {}):
  { kind: "direct"; mode: "shielded" | "transparent"; payment: ShieldedPayment } | { kind: "handoff"; uri: string } {
  const mode = directPaymentMode(request, connector);
  if (mode === "shielded" || (mode === "transparent" && options.allowTransparent === true)) {
    const payment = request.payments[0];
    return { kind: "direct", mode, payment: { to: payment.recipient, amount: zatoshiToZec(payment.amount_zat!) } };
  }
  if (connector.capabilities.has("zip321Handoff") && connector.openPaymentHandoff) {
    return { kind: "handoff", uri: connector.openPaymentHandoff(request.canonical_uri) };
  }
  throw new Error("This wallet cannot preserve the complete payment request. Choose the portable Zcash payment option.");
}
