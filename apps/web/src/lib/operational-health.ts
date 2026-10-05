export type OperationalHealthSnapshot = {
  healthy: boolean;
  attention_required: boolean;
  pending_verifications: number;
  overdue_verifications: number;
  webhook_pending: number;
  webhook_dead: number;
  maintenance_last_success_at: string | null;
  maintenance_stale: boolean;
  zcash: {
    configured: boolean;
    network: string;
    available: boolean | null;
    synced: boolean | null;
    checked_at: string | null;
    last_success_at: string | null;
    stale: boolean;
  };
};

export function operationalProbeStatus(snapshot: OperationalHealthSnapshot): 200 | 503 {
  return snapshot.healthy && !snapshot.attention_required ? 200 : 503;
}
