export type ProposalAgentWriteGate = {
  closed: boolean;
};

export function createProposalAgentWriteGate(): ProposalAgentWriteGate {
  return { closed: false };
}

export function closeProposalAgentWriteGate(gate: ProposalAgentWriteGate): void {
  gate.closed = true;
}

export function mayApplyAgentBatch(gate: ProposalAgentWriteGate): boolean {
  return !gate.closed;
}
