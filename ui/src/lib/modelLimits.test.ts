import { describe, expect, test } from "vitest";
import { getModelContextLimit, getUsageLevel, getLastAssistantUsage } from "./modelLimits";

describe("getModelContextLimit", () => {
  test("returns 1M for opus models", () => {
    expect(getModelContextLimit("claude-opus-4-6-20251001")).toBe(1_000_000);
    expect(getModelContextLimit("claude-opus-4-7")).toBe(1_000_000);
  });

  test("returns 200k for sonnet and haiku models", () => {
    expect(getModelContextLimit("claude-sonnet-4-6-20251001")).toBe(200_000);
    expect(getModelContextLimit("claude-haiku-4-5-20251001")).toBe(200_000);
  });

  test("returns 1M for extended context models", () => {
    expect(getModelContextLimit("claude-opus-4-6[1m]")).toBe(1_000_000);
    expect(getModelContextLimit("claude-opus-4-6-1m")).toBe(1_000_000);
    expect(getModelContextLimit("claude-sonnet-4-6[1m]")).toBe(1_000_000);
  });

  test("handles bare family names", () => {
    expect(getModelContextLimit("opus")).toBe(1_000_000);
    expect(getModelContextLimit("sonnet")).toBe(200_000);
    expect(getModelContextLimit("haiku")).toBe(200_000);
  });

  test("returns null for unrecognized model", () => {
    expect(getModelContextLimit("gpt-4o")).toBeNull();
    expect(getModelContextLimit(null)).toBeNull();
    expect(getModelContextLimit(undefined)).toBeNull();
    expect(getModelContextLimit("")).toBeNull();
  });
});

describe("getUsageLevel", () => {
  test("low when < 50%", () => {
    expect(getUsageLevel(0)).toBe("low");
    expect(getUsageLevel(0.3)).toBe("low");
    expect(getUsageLevel(0.49)).toBe("low");
  });

  test("medium when 50-80%", () => {
    expect(getUsageLevel(0.5)).toBe("medium");
    expect(getUsageLevel(0.65)).toBe("medium");
    expect(getUsageLevel(0.79)).toBe("medium");
  });

  test("high when >= 80%", () => {
    expect(getUsageLevel(0.8)).toBe("high");
    expect(getUsageLevel(0.95)).toBe("high");
    expect(getUsageLevel(1.0)).toBe("high");
  });
});

describe("getLastAssistantUsage", () => {
  test("sums input_tokens + cache fields for total context", () => {
    const chunks = [
      {
        kind: "ai",
        responses: [
          {
            usage: { input_tokens: 6, cache_read_input_tokens: 0, cache_creation_input_tokens: 201619 },
            model: "claude-opus-4-6",
          },
        ],
      },
    ];
    const result = getLastAssistantUsage(chunks);
    expect(result).not.toBeNull();
    expect(result!.inputTokens).toBe(201625);
    expect(result!.contextLimit).toBe(1_000_000);
    expect(result!.ratio).toBeCloseTo(0.2016, 3);
  });

  test("handles cache_read_input_tokens", () => {
    const chunks = [
      {
        kind: "ai",
        responses: [
          {
            usage: { input_tokens: 0, cache_read_input_tokens: 150000, cache_creation_input_tokens: 0 },
            model: "claude-sonnet-4-6",
          },
        ],
      },
    ];
    const result = getLastAssistantUsage(chunks);
    expect(result!.inputTokens).toBe(150000);
    expect(result!.contextLimit).toBe(200_000);
    expect(result!.ratio).toBeCloseTo(0.75);
  });

  test("infers 1M limit when total exceeds detected limit", () => {
    const chunks = [
      {
        kind: "ai",
        responses: [
          {
            usage: { input_tokens: 5, cache_read_input_tokens: 300000, cache_creation_input_tokens: 0 },
            model: "claude-sonnet-4-6",
          },
        ],
      },
    ];
    const result = getLastAssistantUsage(chunks);
    expect(result!.inputTokens).toBe(300005);
    expect(result!.contextLimit).toBe(1_000_000);
  });

  test("works with only input_tokens (no cache fields)", () => {
    const chunks = [
      {
        kind: "ai",
        responses: [
          { usage: { input_tokens: 120000 }, model: "claude-sonnet-4-6-20251001" },
        ],
      },
    ];
    const result = getLastAssistantUsage(chunks);
    expect(result!.inputTokens).toBe(120000);
    expect(result!.contextLimit).toBe(200_000);
    expect(result!.ratio).toBeCloseTo(0.6);
  });

  test("returns null when no AI chunks", () => {
    const chunks = [{ kind: "user", responses: undefined }];
    expect(getLastAssistantUsage(chunks)).toBeNull();
  });

  test("returns null when usage is null", () => {
    const chunks = [
      {
        kind: "ai",
        responses: [{ usage: null, model: "claude-sonnet-4-6" }],
      },
    ];
    expect(getLastAssistantUsage(chunks)).toBeNull();
  });

  test("returns null when model is unrecognized", () => {
    const chunks = [
      {
        kind: "ai",
        responses: [{ usage: { input_tokens: 1000 }, model: "gpt-4o" }],
      },
    ];
    expect(getLastAssistantUsage(chunks)).toBeNull();
  });

  test("skips chunks with zero total tokens", () => {
    const chunks = [
      {
        kind: "ai",
        responses: [
          { usage: { input_tokens: 80000 }, model: "claude-opus-4-6" },
        ],
      },
      {
        kind: "ai",
        responses: [
          { usage: { input_tokens: 0, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 }, model: "claude-opus-4-6" },
        ],
      },
    ];
    const result = getLastAssistantUsage(chunks);
    expect(result!.inputTokens).toBe(80000);
  });
});
