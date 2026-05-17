import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, test, vi } from "vitest";

import ContextPanel from "../../ContextPanel.svelte";
import { multiProjectRichFixture } from "../../../lib/__fixtures__/multi-project-rich";

afterEach(() => {
  cleanup();
});

const detail = multiProjectRichFixture.sessionDetails["mock-rich-rust:sess-rust-active"]!;

function renderPanel() {
  return render(ContextPanel, {
    props: {
      detail,
      onClose: vi.fn(),
      onNavigateToChunk: vi.fn(),
      onNavigateToTool: vi.fn(),
      onNavigateToUserGroup: vi.fn(),
    },
  });
}

describe("ContextPanel", () => {
  test("renders all category sections from rich fixture", () => {
    renderPanel();

    expect(screen.getByText("User Messages")).toBeInTheDocument();
    expect(screen.getByText("CLAUDE.md Files")).toBeInTheDocument();
    expect(screen.getByText("Mentioned Files")).toBeInTheDocument();
    expect(screen.getByText("Tool Outputs")).toBeInTheDocument();
    expect(screen.getByText("Task Coordination")).toBeInTheDocument();
    expect(screen.getByText("Thinking + Text")).toBeInTheDocument();
  });

  test("renders expected rows inside section bodies", async () => {
    renderPanel();

    expect(screen.getByText("继续往下")).toBeInTheDocument();
    expect(screen.getByText("ContextPanel.svelte")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: /1 tool/ }));
    expect(screen.getByText("Bash")).toBeInTheDocument();

    expect(screen.getByText("Task #1: rename audit")).toBeInTheDocument();
    expect(screen.getByText("thinking")).toBeInTheDocument();
  });

  test("shows phase selector only for multi-phase detail", () => {
    const { rerender } = renderPanel();
    expect(screen.getByLabelText("Phase:")).toBeInTheDocument();

    void rerender({
      detail: {
        ...detail,
        phaseInfo: {
          phases: [],
          compactionCount: 0,
          aiGroupPhaseMap: {},
          compactionTokenDeltas: {},
        },
      },
      onClose: vi.fn(),
      onNavigateToChunk: vi.fn(),
      onNavigateToTool: vi.fn(),
      onNavigateToUserGroup: vi.fn(),
    });

    expect(screen.queryByLabelText("Phase:")).not.toBeInTheDocument();
  });
});
