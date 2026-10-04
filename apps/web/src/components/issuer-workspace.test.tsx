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
