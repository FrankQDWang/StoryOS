# Support file dispositions

This ledger is partial. Support files have no runtime test verdict unless they declare a test. Locations refer to the fixed audit baseline.

| File | Disposition | Consumers and reason | Direct removable lines |
|---|---|---|---|
| apps/web/test/node-postgresql/export-acknowledgement-support.ts | KEEP support | assertExportAdmissionFreezes at :168 and assertExportHistoricalEvidence at :261 are called by retained Archive admission tests :815/:842 and readable admission tests :688/:715. They execute the shared test sequence against separate command-specific replay implementations. No standalone tests. | 0 |
