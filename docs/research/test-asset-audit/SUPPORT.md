# Support file dispositions

This ledger is partial. Support files have no runtime test verdict unless they declare a test. Locations refer to the fixed audit baseline.

| File | Disposition | Consumers and reason | Direct removable lines |
|---|---|---|---|
| apps/web/test/node-postgresql/export-acknowledgement-support.ts | KEEP support | assertExportAdmissionFreezes at :168 and assertExportHistoricalEvidence at :261 are called by retained Archive admission tests :815/:842 and readable admission tests :688/:715. They execute the shared test sequence against separate command-specific replay implementations. No standalone tests. | 0 |
| apps/web/test/support/refused-edit-recovery.ts | KEEP support | retainRefusedEditRecoveryExpectation at :6 is called by retained Inline HTTP cases :264,427,485,637,807,1400,1499,1949. With STORYOS_REFUSED_EDIT_RECOVERY_EXPECTED set, it writes scoped before-backup state and query/export expectations consumed by scripts/inspect-recovered-refused-edit-drafts.mjs. The consumer checks 15 Projects and 20 Drafts; it is not a dead local assertion helper. | 0 |
