import { describe, test, expect, vi } from "vitest";
import { activateOnKey } from "./a11y";

function keyEvent(key: string): KeyboardEvent & { preventDefault: ReturnType<typeof vi.fn> } {
  return {
    key,
    preventDefault: vi.fn(),
  } as unknown as KeyboardEvent & { preventDefault: ReturnType<typeof vi.fn> };
}

describe("activateOnKey", () => {
  test("Enter 触发 action 并 preventDefault", () => {
    const action = vi.fn();
    const e = keyEvent("Enter");
    activateOnKey(e, action);
    expect(action).toHaveBeenCalledOnce();
    expect(e.preventDefault).toHaveBeenCalledOnce();
  });

  test("Space 触发 action 并 preventDefault（防页面滚动）", () => {
    const action = vi.fn();
    const e = keyEvent(" ");
    activateOnKey(e, action);
    expect(action).toHaveBeenCalledOnce();
    expect(e.preventDefault).toHaveBeenCalledOnce();
  });

  test("其它键不触发，也不 preventDefault", () => {
    const action = vi.fn();
    for (const key of ["Tab", "a", "ArrowDown", "Escape"]) {
      const e = keyEvent(key);
      activateOnKey(e, action);
      expect(e.preventDefault).not.toHaveBeenCalled();
    }
    expect(action).not.toHaveBeenCalled();
  });
});
