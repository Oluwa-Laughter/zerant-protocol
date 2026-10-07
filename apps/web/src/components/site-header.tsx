import Image from "next/image";
import Link from "next/link";
import { MobileMenu } from "@/components/mobile-menu";

export function SiteHeader() {
  return (
    <header className="site-header">
      <Link href="/" className="brand brand-lockup" aria-label="Zerant home">
        <Image src="/brand/zerant-lockup.svg" alt="Zerant" width={180} height={36} priority />
      </Link>
      <nav aria-label="Main navigation">
        <Link href="/#how-it-works">How it works</Link>
        <Link href="/issuers">Trusted issuers</Link>
        <Link href="/app" className="nav-app">Open Zerant <span aria-hidden="true">→</span></Link>
      </nav>
      <MobileMenu groups={[{ label: "Explore", items: [
        { href: "/", label: "Landing page" },
        { href: "/#how-it-works", label: "How it works" },
        { href: "/issuers", label: "Trusted issuers" },
        { href: "/app", label: "Open Zerant" },
      ] }]} />
    </header>
  );
}
