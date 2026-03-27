import { PredictXError, PredictXErrorCode, parseSorobanError } from "../src/errors";

describe("PredictXError", () => {
  it("sets name to 'PredictXError'", () => {
    const err = new PredictXError(PredictXErrorCode.PollNotFound);
    expect(err.name).toBe("PredictXError");
  });

  it("includes a descriptive message", () => {
    const err = new PredictXError(PredictXErrorCode.PollNotFound);
    expect(err.message).toContain("Poll not found");
  });

  it("appends context to message when provided", () => {
    const err = new PredictXError(PredictXErrorCode.Unauthorized, "only admin");
    expect(err.message).toContain("only admin");
  });

  it("exposes the numeric code", () => {
    const err = new PredictXError(PredictXErrorCode.StakeBelowMinimum);
    expect(err.code).toBe(PredictXErrorCode.StakeBelowMinimum);
  });
});

describe("parseSorobanError", () => {
  it("returns PredictXError as-is", () => {
    const err = new PredictXError(PredictXErrorCode.AlreadyStaked);
    expect(parseSorobanError(err)).toBe(err);
  });

  it("parses contract error code from Soroban message format", () => {
    const raw = new Error("HostError: Error(Contract, #11)");
    const parsed = parseSorobanError(raw);
    expect(parsed).toBeInstanceOf(PredictXError);
    expect((parsed as PredictXError).code).toBe(PredictXErrorCode.AlreadyStaked);
  });

  it("wraps unknown errors in a generic Error", () => {
    const parsed = parseSorobanError("unexpected string error");
    expect(parsed).toBeInstanceOf(Error);
    expect(parsed.message).toContain("unexpected string error");
  });

  it("returns the original Error when code is unrecognised", () => {
    const raw = new Error("Some network error");
    const parsed = parseSorobanError(raw);
    expect(parsed).toBe(raw);
  });
});
