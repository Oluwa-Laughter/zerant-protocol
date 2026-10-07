"use client";

import Image from "next/image";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { SiteHeader } from "@/components/site-header";
import { MobileMenu } from "@/components/mobile-menu";

const productRoutes = ["/app", "/vault", "/requests", "/activity", "/zcash", "/issuer", "/verifier", "/account"];

const primaryItems = [
  { href: "/app", label: "Home" },
  { href: "/vault", label: "My credentials" },
  { href: "/requests", label: "Verification requests" },
  { href: "/activity", label: "Activity" },
  { href: "/zcash", label: "Zcash payments" },
];

const organizationItems = [
  { href: "/issuer", label: "Issue credentials" },
  { href: "/verifier", label: "Request proof" },
];

function isProductRoute(pathname: string): boolean {
  return productRoutes.some((route) => pathname === route || pathname.startsWith(route + "/"));
}

function NavItem({ href, label, pathname }: { href: string; label: string; pathname: string }) {
  const active = pathname === href || (href !== "/app" && pathname.startsWith(href + "/"));
  return <Link className={active ? "product-nav-link active" : "product-nav-link"} href={href} aria-current={active ? "page" : undefined}>{label}</Link>;
}

export function ProductShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const product = isProductRoute(pathname);

  if (!product) {
    return (
      <div className="site-wrap">
        <SiteHeader />
        {children}
        <footer className="footer">
          <span>zerant. <span className="muted">Trust, with boundaries.</span></span>
          <span>Private trust for the Zcash ecosystem · Testnet</span>
        </footer>
      </div>
    );
  }

  return (
    <div className="product-shell">
      <aside className="product-sidebar" aria-label="Zerant workspace navigation">
        <Link href="/app" className="product-sidebar-brand" aria-label="Zerant workspace home">
          <Image src="/brand/zerant-lockup.svg" alt="Zerant" width={154} height={31} priority />
        </Link>
        <p className="product-network-label">Built for Zcash <span>Testnet</span></p>

        <nav className="product-sidebar-nav" aria-label="Workspace">
          <div className="product-nav-group">
            <span className="product-nav-label">Your workspace</span>
            {primaryItems.map((item) => <NavItem key={item.href} {...item} pathname={pathname} />)}
          </div>
          <div className="product-nav-group">
            <span className="product-nav-label">Organizations</span>
            {organizationItems.map((item) => <NavItem key={item.href} {...item} pathname={pathname} />)}
          </div>
        </nav>

        <div className="product-sidebar-bottom">
          <NavItem href="/account" label="Account settings" pathname={pathname} />
          <Link className="product-nav-link product-back-home" href="/">Back to landing page</Link>
        </div>
      </aside>
      <div className="product-shell-main">
        <header className="product-mobile-header">
          <div className="product-mobile-brand-group">
            <Link href="/app" className="product-mobile-brand" aria-label="Zerant workspace home">
              <Image src="/brand/zerant-lockup.svg" alt="Zerant" width={132} height={27} priority />
            </Link>
            <span className="product-mobile-network">Zcash testnet</span>
          </div>
          <MobileMenu signOut groups={[
            { label: "Your workspace", items: primaryItems },
            { label: "Organizations", items: organizationItems },
            { label: "Account", items: [
              { href: "/account", label: "Account settings" },
              { href: "/", label: "Back to landing page" },
            ] },
          ]} />
        </header>
        <div className="product-content">{children}</div>
      </div>
    </div>
  );
}
