---
status: accepted
---

# Trust the StoryOS Web Client for Author Command Admission

Release 1 includes the controlled, deployed StoryOS Web Client in the trusted computing boundary for author-owned commands. The StoryOS Server derives the authenticated User and Project Scope and durably binds the trusted client session, action class, exact command digest, target, expected Heads, nonce, idempotency record, lifetime, and Core settlement in one `AuthorCommandAdmission`; this preserves a responsive browser-first editor while making the precise security claim and its evidence explicit.
