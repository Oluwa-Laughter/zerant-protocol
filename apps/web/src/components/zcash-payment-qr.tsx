"use client";

import { useEffect, useRef, useState } from "react";
import QRCode from "qrcode";

export function ZcashPaymentQr({ uri }: { uri: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    const target = canvas.current;
    if (!target || !uri.startsWith("zcash:")) return;
    let active = true;
    void QRCode.toCanvas(target, uri, {
      width: 224,
      margin: 3,
      errorCorrectionLevel: "M",
      color: { dark: "#182b28", light: "#ffffff" },
    }).then(() => { if (active) setError(false); }, () => { if (active) setError(true); });
    return () => { active = false; };
  }, [uri]);

  return <div className="zcash-payment-qr">
    <canvas ref={canvas} role="img" aria-label="QR code for the reviewed Zcash payment request" />
    {error ? <p className="small muted">This request is too long for a QR code. Copy the complete payment link instead.</p> : null}
  </div>;
}
