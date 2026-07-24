import type { AgentState } from "../types";
import { Creature } from "./Creature";

export function TamagotchiView({ agents }: { agents: AgentState[] }) {
  return (
    <div className="grid grid-cols-3 gap-3">
      {agents.map((a) => (
        <Creature key={a.session_id} status={a.status} name={a.name} />
      ))}
      {agents.length === 0 && (
        <div className="text-neutral-500 col-span-3">No active agents.</div>
      )}
    </div>
  );
}
