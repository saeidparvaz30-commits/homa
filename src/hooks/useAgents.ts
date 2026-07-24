import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AgentState } from "../types";

export function useAgents(): AgentState[] {
  const [agents, setAgents] = useState<AgentState[]>([]);
  useEffect(() => {
    invoke<AgentState[]>("get_agents").then(setAgents).catch(() => {});
    const un = listen<AgentState[]>("agents-updated", (e) => setAgents(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);
  return agents;
}
