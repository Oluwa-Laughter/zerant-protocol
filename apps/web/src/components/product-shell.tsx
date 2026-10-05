"use client";

import Image from "next/image";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { SiteHeader } from "@/components/site-header";

const productRoutes = ["/app", "/vault", "/requests", "/activity", "/zcash", "/issuer", "/verifier", "/account"];

const primaryItems = [
  { href: "/app", label: "Overview" },
  { href: "/vault", label: "Credentials" },
  { href: "/requests", label: "Requests" },
  { href: "/activity", label: "Activity" },
  { href: "/zcash", label: "Zcash" },
];

const organizationItems = [
  { href: "/issuer", label: "Issue credentials" },
  { href: "/verifier", label: "Verify privately" },
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
          <span>Privacy-preserving trust infrastructure</span>
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

        <nav className="product-sidebar-nav" aria-label="Workspace">
          <div className="product-nav-group">
            <span className="product-nav-label">Workspace</span>
            {primaryItems.map((item) => <NavItem key={item.href} {...item} pathname={pathname} />)}
          </div>
          <div className="product-nav-group">
            <span className="product-nav-label">Organizations</span>
            {organizationItems.map((item) => <NavItem key={item.href} {...item} pathname={pathname} />)}
          </div>
        </nav>

        <div className="product-sidebar-bottom">
          <NavItem href="/account" label="Account" pathname={pathname} />
          <Link className="product-nav-link product-back-home" href="/">Back to zerant.com</Link>
        </div>
      </aside>
      <div className="product-shell-main">
        <header className="product-mobile-header">
          <Link href="/app" className="product-mobile-brand" aria-label="Zerant workspace home">
            <Image src="/brand/zerant-lockup.svg" alt="Zerant" width={132} height={27} priority />
          </Link>
          <Link href="/account" className="product-mobile-account">Account</Link>
        </header>
        <div className="product-mobile-nav" aria-label="Workspace navigation">
          {[...primaryItems, ...organizationItems].map((item) => <NavItem key={item.href} {...item} pathname={pathname} />)}
        </div>
        <div className="product-content">{children}</div>
      </div>
    </div>
  );
}
