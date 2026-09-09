---
status: accepted
---

# Own Foundation Validation Public HTTPS Transport Without Advancing PASS-CLOUD

Foundation Validation needs one public `https` Host on the paired VPS so the author can open the printed origin. That transport is not the later controlled-cloud handoff (`EV-CCD`, `HND-005`, `PASS-CLOUD`). This review owns only the public transport profile: one Host, TLS in front of the Server, and cookie `Secure` on that profile. Local loopback HTTP remains the `make verify` oracle.

## Considered options

- Bringing the full later controlled-cloud evidence gate into this owner was rejected. That gate is a multi-user cloud deployment handoff. It would hide the launch-path gap behind a larger campaign.
- Making a public HTTPS browser journey a required `make verify` check was rejected. The current exact-dist journey already proves the paired host, CSP, Trusted Types, and cookie issuance on loopback HTTP. A public Host and certificate are operator identity, not package bytes.
- Leaving Host and Origin equal to the bind address, and documenting Caddy only, was rejected. Admission compares the exact Host header and a WHATWG Origin. A public `https` browser cannot match `http://127.0.0.1:3000`.

## Consequences

- [Own Public Host, HTTPS Origin, and Cookie Secure](https://github.com/FrankQDWang/StoryOS/issues/604) is the originating review. It is not an implementation Claim.
- ADR 0016 still owns paired same-origin delivery. ADR 0022 still places a TLS reverse proxy such as Caddy in front of the Server and rejects Vercel.
- Cookie `Secure` belongs to the public `https` profile. The local HTTP profile keeps the current cookie without `Secure`.
- This decision does not resume Stage 3 and does not absorb the recovery-chain spike.
