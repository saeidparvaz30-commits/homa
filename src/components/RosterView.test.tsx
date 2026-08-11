import { render, screen } from "@testing-library/react";
import { RosterView } from "./RosterView";
import type { AgentState } from "../types";

const mk = (over: Partial<AgentState>): AgentState => ({
  pid: 1,
  session_id: "s",
  name: "n",
  cwd: "c",
  repo: "myrepo",
  branch: "main",
  status: "idle",
  raw_status: "idle",
  started_at: 0,
  status_updated_at: 0,
  model: "claude-fable-5",
  context_pct: 50,
  last_activity: null,
  ended_at: null,
  ...over,
});

test("groups by repo and shows name + status", () => {
  render(<RosterView agents={[mk({ name: "alpha", status: "waiting" })]} />);
  expect(screen.getByText("alpha")).toBeInTheDocument();
  expect(screen.getByText("myrepo")).toBeInTheDocument();
  expect(screen.getByText(/waiting/i)).toBeInTheDocument();
});
