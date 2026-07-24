---
status: accepted
---

# Trust the StoryOS Web Client for Author Command Admission

Release 1 includes only the exact controlled StoryOS Web Client build named by its immutable asset set, accepted client-contract and security-policy identities, and current protected Client Session Binding generation in the trusted computing boundary for author-owned commands. Rendered or imported content, model, Tool, MCP, or App output, browser extensions, third-party scripts, browser-local caches, journals, and projections are not part of that trusted claim.

The StoryOS Server derives the authenticated User and exact existing or prospective Project Scope and binds the protected client session, accepted client-contract and security-policy identities, applicable Editor Session and writer generation, action class, exact command digest, targets, expected Heads, nonce, idempotency record, lifetime, and unique terminal settlement in one `AuthorCommandAdmission`. This preserves a responsive browser-first editor while proving only an exact protected-client submission, not one physical human gesture, trusted display, user presence, or user verification; stronger transaction confirmation requires a separately trusted display and confirmation surface outside the selected Release 1 boundary.
