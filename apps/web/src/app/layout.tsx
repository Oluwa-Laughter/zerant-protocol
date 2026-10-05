import type { Metadata } from "next";
import { ProductShell } from "@/components/product-shell";
import "./globals.css";

export const metadata: Metadata = {
  title: {
    default: "Zerant — Prove trust. Preserve privacy.",
    template: "%s | Zerant",
  },
  description:
    "Privacy-preserving trust infrastructure for credentials, consent, contextual policy and Zcash settlement.",
  icons: {
    icon: "/brand/zerant-mark.svg",
    shortcut: "/brand/zerant-mark.svg",
  },
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>
        <a className="skip-link" href="#main">Skip to content</a>
        <ProductShell>{children}</ProductShell>
      </body>
    </html>
  );
}
