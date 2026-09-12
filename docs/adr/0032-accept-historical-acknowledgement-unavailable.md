# Accept Historical Acknowledgement Unavailable

A known pre-capture command may lack the stored Project projection needed to replay its original acknowledgement. Reconstructing that reply from live Project state, or fabricating a success body, was rejected. The accepted exception is HTTP 409 `historical_acknowledgement_unavailable`: the original reply cannot be recovered, and the author must refresh to inspect current state.

## Considered options

- Reading the live Project on retry was rejected. A later rename or Current Chapter change would contaminate the earlier acknowledgement.
- Reconstructing every historical reply from Activity or Receipt fragments was rejected. Those records do not prove the complete original Project fields for pre-capture rows.
- Resetting development data or backfilling live state into old keys was rejected. Existing novels, Receipts, and idempotency evidence must survive the additive storage change.
- Treating missing new-format evidence as the same historical exception was rejected. A damaged `command_response_project.v1` row is a storage fault.
