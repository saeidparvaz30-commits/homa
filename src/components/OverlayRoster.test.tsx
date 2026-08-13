import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { OverlayRoster } from "./OverlayRoster";
import type { AgentState } from "../types";

// vi.mock factories are hoisted above the file's own declarations, so the
// mutable fixture has to be created inside vi.hoisted or the factory hits
// a temporal dead zone error on `mockAgents`.
const h = vi.hoisted(() => ({ agents: [] as AgentState[] }));

vi.mock("../hooks/useAgents", () => ({ useAgents: () => h.agents }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setSize: vi.fn().mockResolvedValue(undefined),
    startDragging: vi.fn(),
  }),
}));
vi.mock("@tauri-apps/api/dpi", () => ({
  LogicalSize: class { constructor(public width: number, public height: number) {} },
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

const setAgents = (a: AgentState[]) => {
  h.agents = a;
};

const mk = (over: Partial<AgentState>): AgentState => ({
  pid: 1,
  session_id: "s",
  name: "n",
  cwd: "c",
  repo: "r",
  branch: null,
  status: "working",
  raw_status: "busy",
  started_at: 0,
  status_updated_at: 0,
  model: null,
  context_pct: null,
  last_activity: null,
  ended_at: null,
  limited_until: null,
  was_busy_at_limit: false,
  resume_fired: false,
  ...over,
});

test("renders one row per session showing its name", () => {
  setAgents([
    mk({ session_id: "a", name: "migration site", status: "waiting" }),
    mk({ session_id: "b", name: "homa", status: "working" }),
  ]);
  render(<OverlayRoster />);
  expect(screen.getByText("migration site")).toBeInTheDocument();
  expect(screen.getByText("homa")).toBeInTheDocument();
});

test("orders waiting above idle above working", () => {
  setAgents([
    mk({ session_id: "a", name: "third", status: "working" }),
    mk({ session_id: "b", name: "first", status: "waiting" }),
    mk({ session_id: "c", name: "second", status: "idle" }),
  ]);
  render(<OverlayRoster />);
  const names = screen.getAllByTestId("row-name").map((n) => n.textContent);
  expect(names).toEqual(["first", "second", "third"]);
});

test("shows a muted empty state rather than nothing", () => {
  setAgents([]);
  render(<OverlayRoster />);
  expect(screen.getByText(/no sessions/i)).toBeInTheDocument();
});

test("marks each row with its status for colouring", () => {
  setAgents([mk({ session_id: "a", name: "x", status: "waiting" })]);
  render(<OverlayRoster />);
  expect(screen.getByTestId("row-dot")).toHaveAttribute("data-status", "waiting");
});

const oneRow = () => setAgents([mk({ session_id: "a", name: "homa", cwd: "C:\\Homa" })]);

const edit = (value: string) => {
  render(<OverlayRoster />);
  fireEvent.doubleClick(screen.getByTestId("row-name"));
  const box = screen.getByRole("textbox");
  fireEvent.change(box, { target: { value } });
  return box;
};

test("double click turns the name into an input seeded with the current name", () => {
  oneRow();
  render(<OverlayRoster />);
  fireEvent.doubleClick(screen.getByTestId("row-name"));
  expect(screen.getByRole("textbox")).toHaveValue("homa");
});

test("enter commits the new name against the row's session", () => {
  vi.mocked(invoke).mockClear();
  oneRow();
  fireEvent.keyDown(edit("tray app"), { key: "Enter" });
  expect(invoke).toHaveBeenCalledWith("set_alias", { sessionId: "a", name: "tray app" });
});

test("escape cancels without saving", () => {
  vi.mocked(invoke).mockClear();
  oneRow();
  fireEvent.keyDown(edit("discarded"), { key: "Escape" });
  expect(invoke).not.toHaveBeenCalled();
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
});

test("an empty commit clears the alias rather than storing a blank", () => {
  vi.mocked(invoke).mockClear();
  oneRow();
  fireEvent.keyDown(edit("   "), { key: "Enter" });
  expect(invoke).toHaveBeenCalledWith("set_alias", { sessionId: "a", name: "   " });
});

test("limited row shows purple dot and reset time", () => {
  const at = new Date(2026, 7, 13, 0, 40).getTime();
  setAgents([mk({ session_id: "a", name: "x", status: "limited", limited_until: at })]);
  render(<OverlayRoster />);
  expect(screen.getByTestId("row-dot")).toHaveAttribute("data-status", "limited");
  expect(screen.getByText("resets 12:40am")).toBeInTheDocument();
});

test("limited row past reset with resume fired says resuming", () => {
  setAgents([
    mk({ session_id: "a", name: "x", status: "limited", limited_until: 5, resume_fired: true }),
  ]);
  render(<OverlayRoster />);
  expect(screen.getByText("resuming")).toBeInTheDocument();
});

test("single click focuses the session terminal after the dblclick window", () => {
  vi.useFakeTimers();
  vi.mocked(invoke).mockClear();
  setAgents([mk({ session_id: "a", name: "homa", pid: 7, cwd: "C:\\Homa" })]);
  render(<OverlayRoster />);
  fireEvent.click(screen.getByTestId("row-name"));
  expect(invoke).not.toHaveBeenCalled();
  vi.advanceTimersByTime(300);
  expect(invoke).toHaveBeenCalledWith("focus_session", {
    pid: 7,
    cwd: "C:\\Homa",
    sessionId: "a",
  });
  vi.useRealTimers();
});

test("double click renames and cancels the pending focus", () => {
  vi.useFakeTimers();
  vi.mocked(invoke).mockClear();
  setAgents([mk({ session_id: "a", name: "homa", cwd: "C:\\Homa" })]);
  render(<OverlayRoster />);
  fireEvent.click(screen.getByTestId("row-name"));
  fireEvent.doubleClick(screen.getByTestId("row-name"));
  vi.advanceTimersByTime(600);
  expect(invoke).not.toHaveBeenCalledWith("focus_session", expect.anything());
  expect(screen.getByRole("textbox")).toBeInTheDocument();
  vi.useRealTimers();
});

test("top bar shows a minimize button that hides the overlay", () => {
  vi.mocked(invoke).mockClear();
  setAgents([]);
  render(<OverlayRoster />);
  fireEvent.click(screen.getByLabelText("minimize"));
  expect(invoke).toHaveBeenCalledWith("hide_overlay");
});

test("blur commits", () => {
  vi.mocked(invoke).mockClear();
  oneRow();
  fireEvent.blur(edit("blurred"));
  expect(invoke).toHaveBeenCalledWith("set_alias", { sessionId: "a", name: "blurred" });
});
