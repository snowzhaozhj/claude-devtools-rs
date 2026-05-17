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


  test("switches Ranked grouped and flat modes", async () => {
    renderPanel();

    await fireEvent.click(screen.getByRole("button", { name: /By Size/ }));
    expect(screen.getByRole("button", { name: "Grouped" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Flat" })).toBeInTheDocument();
    expect(screen.getByText("Tool")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Flat" }));
    expect(screen.getByText("Bash")).toBeInTheDocument();
    expect(screen.getByText("ContextPanel.svelte")).toBeInTheDocument();
  });

  test("phase selector filters content and shows empty phase message", async () => {
    const detailWithEmptyPhase = {
      ...detail,
      phaseInfo: {
        ...detail.phaseInfo!,
        phases: [
          ...detail.phaseInfo!.phases,
          { phaseNumber: 3, firstAiGroupId: "a-empty:0", lastAiGroupId: "a-empty:0" },
        ],
      },
      injectionsByPhase: {
        ...detail.injectionsByPhase!,
        "3": [],
      },
    };
    render(ContextPanel, {
      props: {
        detail: detailWithEmptyPhase,
        onClose: vi.fn(),
        onNavigateToChunk: vi.fn(),
        onNavigateToTool: vi.fn(),
        onNavigateToUserGroup: vi.fn(),
      },
    });

    await fireEvent.change(screen.getByLabelText("Phase:"), { target: { value: "1" } });
    expect(screen.getByText("LocalDataApi 的 list_sessions 用 camelCase 还是 snake_case？")).toBeInTheDocument();
    expect(screen.queryByText("ContextPanel.svelte")).not.toBeInTheDocument();

    await fireEvent.change(screen.getByLabelText("Phase:"), { target: { value: "3" } });
    expect(screen.getByText("本 phase 无 injection")).toBeInTheDocument();
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
