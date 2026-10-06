import { cookies } from "next/headers";
import { fetchZerantBackend } from "@/lib/server-api";
import type {
  CredentialSchema,
  IssuedCredential,
  IssuerActivityPage,
  IssuerInvitation,
  IssuerKeyView,
  IssuerMember,
  IssuerProfile,
} from "@/components/issuer-workspace";

export type IssuerWorkspaceData = {
  backendAvailable: boolean;
  authenticated: boolean;
  currentZerantId: string | null;
  profile: IssuerProfile | null;
  issued: IssuedCredential[];
  schemas: CredentialSchema[];
  keys: IssuerKeyView[];
  team: IssuerMember[];
  teamInvitations: IssuerInvitation[];
  myInvitations: IssuerInvitation[];
  activity: IssuerActivityPage;
};

export async function loadIssuerWorkspaceData(): Promise<IssuerWorkspaceData> {
  const cookieHeader = (await cookies()).toString();
  let backendAvailable = false;
  let authenticated = false;
  let currentZerantId: string | null = null;
  let profile: IssuerProfile | null = null;
  let issued: IssuedCredential[] = [];
  let schemas: CredentialSchema[] = [];
  let keys: IssuerKeyView[] = [];
  let team: IssuerMember[] = [];
  let teamInvitations: IssuerInvitation[] = [];
  let myInvitations: IssuerInvitation[] = [];
  let activity: IssuerActivityPage = { items: [], next_cursor: null };

  try {
    const sessionResponse = await fetchZerantBackend("/v1/session", cookieHeader);
    backendAvailable = sessionResponse !== null;
    authenticated = Boolean(sessionResponse?.ok);

    if (sessionResponse?.ok) {
      const session = (await sessionResponse.json()) as { zerant_id: string };
      currentZerantId = session.zerant_id;
      const [profileResponse, myInvitationsResponse] = await Promise.all([
        fetchZerantBackend("/v1/issuer", cookieHeader),
        fetchZerantBackend("/v1/issuer/invitations", cookieHeader),
      ]);

      if (myInvitationsResponse?.ok) myInvitations = await myInvitationsResponse.json() as IssuerInvitation[];
      if (profileResponse?.ok) {
        profile = await profileResponse.json() as IssuerProfile;
        const [issuedResponse, schemasResponse, keysResponse, teamResponse, teamInvitationsResponse, activityResponse] = await Promise.all([
          fetchZerantBackend("/v1/issuer/credentials", cookieHeader),
          fetchZerantBackend("/v1/issuer/schemas", cookieHeader),
          fetchZerantBackend("/v1/issuer/keys", cookieHeader),
          fetchZerantBackend("/v1/issuer/team", cookieHeader),
          fetchZerantBackend("/v1/issuer/team/invitations", cookieHeader),
          fetchZerantBackend("/v1/issuer/activity?limit=20", cookieHeader),
        ]);
        if (issuedResponse?.ok) issued = await issuedResponse.json() as IssuedCredential[];
        if (schemasResponse?.ok) schemas = await schemasResponse.json() as CredentialSchema[];
        if (keysResponse?.ok) keys = await keysResponse.json() as IssuerKeyView[];
        if (teamResponse?.ok) team = await teamResponse.json() as IssuerMember[];
        if (teamInvitationsResponse?.ok) teamInvitations = await teamInvitationsResponse.json() as IssuerInvitation[];
        if (activityResponse?.ok) activity = await activityResponse.json() as IssuerActivityPage;
      }
    }
  } catch {
    backendAvailable = false;
  }

  return { backendAvailable, authenticated, currentZerantId, profile, issued, schemas, keys, team, teamInvitations, myInvitations, activity };
}
