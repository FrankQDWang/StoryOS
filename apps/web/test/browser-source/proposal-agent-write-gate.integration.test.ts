import { expect, it } from "vitest";

import {
  closeProposalAgentWriteGate,
  createProposalAgentWriteGate,
  mayApplyAgentBatch,
} from "../../src/proposal-agent-write-gate.ts";

it("closes the Agent write gate on first author input and does not reopen after compositionend", () => {
  const gate = createProposalAgentWriteGate();
  expect(mayApplyAgentBatch(gate)).toBe(true);
  closeProposalAgentWriteGate(gate);
  expect(mayApplyAgentBatch(gate)).toBe(false);
});
