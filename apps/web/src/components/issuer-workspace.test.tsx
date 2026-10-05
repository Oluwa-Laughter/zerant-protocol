import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import {
  IssuerWorkspace,
  type IssuerMember,
  type IssuerProfile,
} from "./issuer-workspace";

const profile: IssuerProfile = {
  display_name: "Private Trust Org",
  issuer_id: "zerant:issuer:test",
  created_at: "2026-10-04T12:00:00Z",
};

const auditor: IssuerMember = {
  zerant_id: "zr_aaaaaaaaaaaaaaaaaaaaaaaa",
  role: "auditor",
  owner: false,
  joined_at: "2026-10-04T12:00:00Z",
};

test("auditor workspace is read-only", () => {
  const html = renderToStaticMarkup(
    <IssuerWorkspace
      authenticated
      backendAvailable
      currentZerantId={auditor.zerant_id}
      initialProfile={profile}
      initialIssued={[]}
      initialSchemas={[]}
      initialKeys={[]}
      initialTeam={[auditor]}
      initialTeamInvitations={[]}
      initialMyInvitations={[]}
      initialActivity={{ items: [], next_cursor: null }}
    />,
  );

  assert.ok(html.includes("Read-only access"));
  assert.ok(html.includes("Security changes are limited to organization owners and admins."));
  assert.equal(html.includes("Rotate security key"), false);
  assert.equal(html.includes("Send invitation"), false);
  assert.equal(html.includes("Transfer ownership"), false);
});

test("new organization explains who receives a credential and what a wallet does", () => {
  const html = renderToStaticMarkup(
    <IssuerWorkspace
      authenticated
      backendAvailable
      currentZerantId="zr_aaaaaaaaaaaaaaaaaaaaaaaa"
      initialProfile={null}
      initialIssued={[]}
      initialSchemas={[]}
      initialKeys={[]}
      initialTeam={[]}
      initialTeamInvitations={[]}
      initialMyInvitations={[]}
      initialActivity={{ items: [], next_cursor: null }}
    />,
  );

  assert.ok(html.includes("Define a credential type"));
  assert.ok(html.includes("Ask for their Zerant ID"));
  assert.ok(html.includes("No wallet address is needed"));
});

test("issuer must review the recipient and claim before sending", () => {
  const html = renderToStaticMarkup(
    <IssuerWorkspace
      authenticated
      backendAvailable
      currentZerantId="zr_aaaaaaaaaaaaaaaaaaaaaaaa"
      initialProfile={profile}
      initialIssued={[]}
      initialSchemas={[{
        id: "schema-1", issuer_id: profile.issuer_id, issuer_name: profile.display_name,
        display_name: "Membership", description: "Active member", claim_type: "membership",
        context: "community", default_expiry_days: 90, version: 1, active: true,
        supersedes_schema_id: null, retired_at: null, created_at: profile.created_at,
      }]}
      initialKeys={[]}
      initialTeam={[{ ...auditor, role: "owner", owner: true }]}
      initialTeamInvitations={[]}
      initialMyInvitations={[]}
      initialActivity={{ items: [], next_cursor: null }}
    />,
  );

  assert.ok(html.includes("Review credential"));
  assert.ok(html.includes("From organization setup to a live credential."));
  assert.ok(html.includes('href="#credential-types"'));
  assert.ok(html.includes('href="#issue-credential"'));
  assert.ok(html.includes("1 active type"));
  assert.equal(html.includes("Confirm and issue"), false);
});
