import type { AgentState } from "../types";

const BADGE: Record<string, string> = {
  working: "text-sky-400",
  idle: "text-amber-400",
  waiting: "text-red-400",
  ended: "text-neutral-500",
};

export function RosterView({ agents }: { agents: AgentState[] }) {
  const byRepo = agents.reduce<Record<string, AgentState[]>>((m, a) => {
    (m[a.repo] ||= []).push(a);
    return m;
  }, {});
  return (
    <div className="space-y-4">
      {Object.entries(byRepo).map(([repo, list]) => (
        <div key={repo}>
          <div className="text-neutral-300 font-semibold mb-1">{repo}</div>
          <div className="divide-y divide-neutral-800">
            {list.map((a) => (
              <div key={a.session_id} className="flex items-center justify-between py-1.5 text-sm">
                <span className="text-neutral-100">{a.name}</span>
                <span className="text-neutral-500">{a.branch ?? ""}</span>
                <span className="text-neutral-500">
                  {a.context_pct != null ? `${a.context_pct.toFixed(0)}%` : ""}
                </span>
                <span className={`capitalize ${BADGE[a.status]}`}>{a.status}</span>
              </div>
            ))}
          </div>
        </div>
      ))}
      {agents.length === 0 && <div className="text-neutral-500">No active agents.</div>}
    </div>
  );
}
