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
  retired_at: null,
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


test("retired issuer disables new trust creation but preserves maintenance language", () => {
  const owner = { ...auditor, role: "owner" as const, owner: true };
  const html = renderToStaticMarkup(
    <IssuerWorkspace
      authenticated
      backendAvailable
      currentZerantId={owner.zerant_id}
      initialProfile={{ ...profile, retired_at: "2026-10-06T09:00:00Z" }}
      initialIssued={[]}
      initialSchemas={[]}
      initialKeys={[]}
      initialTeam={[owner]}
      initialTeamInvitations={[]}
      initialMyInvitations={[]}
      initialActivity={{ items: [], next_cursor: null }}
    />,
  );
  assert.ok(html.includes("New trust creation is closed."));
  assert.ok(html.includes("Existing credentials, revocation, security keys"));
  assert.equal(html.includes("Send invitation"), false);
  assert.equal(html.includes("Retire issuer"), false);
  assert.ok(html.includes("Transfer ownership before deleting"));
});


test("retired issuer exposes only the admin successor path", () => {
  const retiredProfile = { ...profile, retired_at: "2026-10-06T10:00:00Z" };
  const html = renderToStaticMarkup(<IssuerWorkspace
    authenticated={true}
    backendAvailable={true}
    currentZerantId="zr_111111111111111111111111"
    initialProfile={retiredProfile}
    initialIssued={[]}
    initialSchemas={[]}
    initialKeys={[]}
    initialTeam={[{ zerant_id: "zr_111111111111111111111111", role: "owner", owner: true, joined_at: profile.created_at }]}
    initialTeamInvitations={[]}
    initialMyInvitations={[]}
    initialActivity={{ items: [], next_cursor: null }}
  />);
  assert.ok(html.includes("Ownership succession"));
  assert.ok(html.includes("Admin successor"));
  assert.ok(html.includes("Invite successor"));
  assert.ok(html.includes("retirement does not reopen issuance or configuration"));
  assert.equal(html.includes("Issuer — issue and revoke credentials"), false);
});


test("ownership transfer is only offered to admins", () => {
  const html = renderToStaticMarkup(<IssuerWorkspace
    authenticated={true}
    backendAvailable={true}
    currentZerantId="zr_111111111111111111111111"
    initialProfile={profile}
    initialIssued={[]}
    initialSchemas={[]}
    initialKeys={[]}
    initialTeam={[
      { zerant_id: "zr_111111111111111111111111", role: "owner", owner: true, joined_at: profile.created_at },
      { zerant_id: "zr_222222222222222222222222", role: "admin", owner: false, joined_at: profile.created_at },
      { zerant_id: "zr_333333333333333333333333", role: "issuer", owner: false, joined_at: profile.created_at },
      { zerant_id: "zr_444444444444444444444444", role: "auditor", owner: false, joined_at: profile.created_at },
    ]}
    initialTeamInvitations={[]}
    initialMyInvitations={[]}
    initialActivity={{ items: [], next_cursor: null }}
  />);
  assert.equal((html.match(/Transfer ownership/g) ?? []).length, 1);
  assert.ok(html.includes("only eligible ownership successors"));
});


test("private payout activity does not expose a reusable holder identifier", () => {
  const html = renderToStaticMarkup(<IssuerWorkspace
    authenticated={true}
    backendAvailable={true}
    currentZerantId="zr_111111111111111111111111"
    initialProfile={profile}
    initialIssued={[]}
    initialSchemas={[]}
    initialKeys={[]}
    initialTeam={[{ zerant_id: "zr_111111111111111111111111", role: "owner", owner: true, joined_at: profile.created_at }]}
    initialTeamInvitations={[]}
    initialMyInvitations={[]}
    initialActivity={{
      items: [{
        id: 7,
        event_type: "payout_destination_received",
        actor_zerant_id: "private-holder",
        object_id: "12345678-1234-1234-1234-123456789abc",
        label: "Private payout destination",
        context: "zcash:testnet",
        counterparty: null,
        created_at: profile.created_at,
      }],
      next_cursor: null,
    }}
    activeSection="activity"
  />);

  assert.ok(html.includes("By private holder"));
  assert.equal(html.includes("private-holder"), false);
  assert.equal(html.includes("zr_holder"), false);
});
