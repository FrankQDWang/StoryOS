---
status: accepted
---

# Hold the Allowed Origin on the Server

The packaged Server holds the allowed site as one configured Foundation Validation Public Origin. Forwarded headers do not define that Origin, because a mis-set proxy must not change admission. A reverse proxy may terminate TLS and must present the public Host; it is not the admission authority.

## Considered options

- Trusting forwarded headers from the proxy was rejected. A compromised or mis-set proxy could change the allowed Origin. Most applications do that behind a proxy; StoryOS admission must not.
- Keeping the bind address as the allowed Origin was rejected by ADR 0023.
- Treating a proxied CDN as the TLS owner was rejected. Exact-dist proves the bytes the Server serves. A second terminator can change those bytes. A DNS provider, including Cloudflare gray-cloud DNS, is not the allowed Origin.

## Consequences

- This ADR records the admission-authority decision. The current Server still treats the bind address as the allowed Origin. A later ticket changes the packaged Server.
