---
status: accepted
---

# Own Foundation Validation Public HTTPS Transport Without Advancing PASS-CLOUD

Foundation Validation needs one public `https` Host on the paired VPS so the author can open the printed origin. That transport is not the later controlled-cloud handoff (`EV-CCD`, `HND-005`, `PASS-CLOUD`). This review owns only the public transport profile: one Host, TLS in front of the Server, and cookie `Secure` on that profile. Local loopback HTTP remains the `make verify` oracle.

## Considered options

- Bringing the full later controlled-cloud evidence gate into this owner was rejected. That gate is a multi-user cloud deployment handoff. It would hide the launch-path gap behind a larger campaign.
- Making a public HTTPS browser journey a required `make verify` check was rejected. The current exact-dist journey already proves the paired host, CSP, Trusted Types, and cookie issuance on loopback HTTP. A public Host and certificate are operator identity, not package bytes.
- Leaving Host and Origin equal to the bind address, and documenting Caddy only, was rejected. Admission compares the exact Host header and a WHATWG Origin. A public `https` browser cannot match `http://127.0.0.1:3000`.
- Trusting `X-Forwarded-Host` or `X-Forwarded-Proto` to define the allowed site was rejected. See ADR 0024.
- A public IP as the admitted Host was rejected. The Host is one operator-owned DNS name.
- Binding the Server to a public interface was rejected. The Server listens on loopback or a unix socket. The TLS reverse proxy is the only public listener.
- A proxied CDN, including Cloudflare orange-cloud proxy, was rejected. It is a second Web host: it terminates TLS in front of Caddy and can change served bytes. Cloudflare gray-cloud DNS is only a DNS provider and is allowed.

## Consequences

- [Own Public Host, HTTPS Origin, and Cookie Secure](https://github.com/FrankQDWang/StoryOS/issues/604) is the originating review. It is not an implementation Claim.
- ADR 0016 still owns paired same-origin delivery. ADR 0022 still places a TLS reverse proxy such as Caddy in front of the Server and rejects Vercel.
- Cookie `Secure` belongs to the public `https` profile. The local HTTP profile keeps the current cookie without `Secure`. The cookie stays host-only. It has no `Domain` attribute.
- The operator configures one WHATWG Origin. The Server derives the allowed Host from that Origin. Default https port 443 is omitted. The printed Server URL is that Origin. Extra DNS names, including `www`, redirect at Caddy. The Server admits only the configured Host.
- One binary has two profiles. When the public Origin is unset, today's local HTTP profile remains. When it is set, that Origin is the printed site and a non-loopback listen is refused.
- Caddy is operator-owned. It is not part of the release package. Required behavior: terminate TLS, present the public Host, proxy to the Server listen address, redirect port 80 to `https`, and send HSTS. An example Caddyfile may live in the operations document. HTTP-01 or DNS-01 is operator practice. StoryOS does not take a DNS API token.
- Public HTTPS proof is an operator checklist against the printed origin. It is not a `make verify` journey and not a second Browser Mode harness.
- Packaged `storyos-server` has two profiles. When `STORYOS_PUBLIC_ORIGIN` is absent, Host and Origin come from the bind address and the cookie omits `Secure`. When that setting is one `https` Origin, the Server prints that Origin, derives the Host, issues `Secure`, and refuses a non-loopback listen.
- This decision does not resume Stage 3 and does not absorb the recovery-chain spike.
