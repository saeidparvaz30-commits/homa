export type AgentStatus = "working" | "idle" | "waiting" | "limited" | "ended";

export interface AgentState {
  pid: number;
  session_id: string;
  name: string;
  cwd: string;
  repo: string;
  branch: string | null;
  status: AgentStatus;
  raw_status: string;
  started_at: number;
  status_updated_at: number;
  model: string | null;
  context_pct: number | null;
  last_activity: number | null;
  ended_at: number | null;
  limited_until: number | null;
  was_busy_at_limit: boolean;
  resume_fired: boolean;
}

export interface TraySummary {
  top: AgentStatus;
  waiting: number;
  limited: number;
  idle: number;
  working: number;
}
