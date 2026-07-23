# Browser author-intent attestation boundaries

- Audited: 2026-07-23
- Scope: primary-source evidence for
  [Research Trustworthy Browser Author-Intent Attestation Boundaries](https://github.com/FrankQDWang/StoryOS/issues/67)
- Threat model: the first-party Web Client's page state or script may be
  malicious or compromised, while the browser, authenticator, operating-system
  trusted surface, StoryOS Server, and their cryptography remain within their
  stated trust boundaries
- Evidence policy: current WHATWG/W3C/IETF/FIDO specifications and directly
  relevant first-party browser or platform documentation only
- Authority: research evidence only; this note does not resolve
  [Specify Trustworthy Author Intent Admission](https://github.com/FrankQDWang/StoryOS/issues/68),
  change `CONTEXT.md`, or supersede a Foundation contract

## Executive finding

No surveyed general-purpose Web primitive lets a pure Web Client prove that one
physical author interaction approved the semantic content of one exact StoryOS
Core command when first-party page script is compromised.

The available primitives prove narrower facts:

- HTML user activation proves that a trusted input event recently activated a
  browsing context. It is a browser-local gate, not a signed statement about
  the event target, displayed UI, command, digest, User, or Project.
- Session, Origin, Host, CSRF, TLS, digest, and idempotency controls can bind
  requester context and exact bytes. A compromised same-origin client can still
  use the legitimate session and ask the Server to admit attacker-chosen bytes.
- WebAuthn can prove that a registered credential made an assertion for an RP,
  Origin, and server challenge, with signed user-presence and optionally
  user-verification flags. Plain WebAuthn does not specify trusted display of
  command data placed behind that challenge. Its specification warns that
  malicious code in the credential's RP-origin scope can invalidate all
  WebAuthn security guarantees.
- WebCrypto, HTTP Message Signatures, DPoP, app/key attestation, and request
  integrity can prove key use, message integrity, client/app provenance, or
  possession. None of those facts is human command authorization.
- A command-intent claim resistant to the compromised page needs an independent
  trusted confirmation surface that displays the meaningful command and binds
  the confirmed display to signed, fresh, command-specific data. Secure Payment
  Confirmation demonstrates that shape for payments only. Android Protected
  Confirmation demonstrates it in a native, hardware-backed trusted UI, while
  explicitly limiting confirmation to what was actually displayed.

Therefore a browser-only implementation may honestly record input evidence,
authenticated requester evidence, and even a user-verified WebAuthn ceremony
bound to a command challenge. It must not rename their conjunction as proof
that the physical Project Author saw and authorized that exact command.

### Feasibility classification for the selected delivery boundary

Under the accepted combination of:

1. a pure first-party Web Client,
2. a fully compromised first-party page remaining in scope, and
3. no general-purpose browser transaction-confirmation API or trusted
   native/editor surface in the selected delivery boundary,

the current **Host-attested, exact-command Author Intent** claim is
**not implementable by that boundary**. This is not merely missing code. The
component that observes the interaction and constructs the command is the
compromised component, while the general-purpose trusted browser/authenticator
surfaces do not display and attest the StoryOS command semantics.

This does not decide Release 1. The downstream decision must either change the
delivery/trust boundary, narrow the claim for a precisely named command class,
or change the accepted attacker model; primary sources cannot select among
those product/security choices.

## The proof obligations are different

These terms must remain separate because satisfying one does not imply the
next:

| Boundary | What a verifier may conclude | What remains unproved |
| --- | --- | --- |
| User activation | A qualifying trusted input event recently activated this browsing context | who acted, what UI they saw, or which command the page derived |
| Authentication | A session or registered credential maps to a StoryOS User under the verifier's policy | authorization of this Project command |
| User presence (`UP`) | A person performed a presence test at the authenticator | which person, knowledge of command content, or approval of that content |
| User verification (`UV`) | The authenticator locally verified the same authenticator user across ceremonies, subject to authenticator policy | a concrete natural-person identity or exact-command approval |
| Command/digest binding | Signed or server-held evidence covers one exact canonical command or challenge-to-command mapping | that the human saw or understood those covered bytes |
| Transaction confirmation | A trusted surface displayed defined semantic fields and bound consent to signed confirmation data | facts not displayed, or application semantics outside the confirmation profile |
| Trusted display/input | Page-controlled UI and events cannot substitute the displayed confirmation or confirmation input | requester scope, freshness, replay, and settlement unless separately bound |
| Authenticator/app attestation | A credential, key, device, or app instance has attested provenance or properties | per-command presence, verification, display, or author intent |
| Native bridge | An installed extension/app can exchange messages outside ordinary page APIs | that the native endpoint independently validated or displayed anything |

## Primary-source findings

### 1. HTML user activation is a capability gate, not an attestation

**External facts.** WHATWG HTML tracks sticky and transient activation on a
`Window`. Transient activation lasts at most a few seconds and indicates only
that the user interacted with the window recently. Some APIs consume it, while
other transient-activation-gated APIs permit multiple calls before expiry.
Activation-triggering events must be trusted `keydown`, pointer, mouse, or touch
events, and activation is propagated to ancestor and same-origin descendant
windows.
([HTML, tracking user activation](https://html.spec.whatwg.org/multipage/interaction.html#tracking-user-activation))

**Boundary.** A synthetic script-dispatched event cannot itself create user
activation. Compromised page code can nevertheless run during a real trusted
event, inspect `navigator.userActivation.isActive`, spend that activation on
another eligible operation, or directly construct a different Core request.
The activation state exposes only booleans; there is no server-verifiable
statement binding the event, DOM target, visible text, command digest, or
Project.

**StoryOS implication, not a decision.** Trusted browser events remain useful
for the existing `Editor Verification Split` as **browser-observed interaction
and cardinality evidence**: browser tests can establish input normalization
behavior in the implementation under test. Under this ticket's attacker model
they cannot be the authority oracle, adversarial intent attestation, or the
current glossary's `Host-attested` Author Intent.

### 2. Request authentication and anti-forgery do not prove author choice

**External facts.** TLS authenticates the configured server endpoint and
protects the transport under its assumptions. HTTP Digest Fields can detect
content corruption, but RFC 9530 says they are not a general malicious-tampering
defense without TLS or signatures.
([RFC 9530 section 6](https://www.rfc-editor.org/rfc/rfc9530.html#section-6))
HTTP Message Signatures can prove that an identified key signed a defined set
of semantically equivalent HTTP components. The application still has to
define covered components, signer-key meaning, freshness, and authorization;
message signatures alone are explicitly only a toolkit, not a complete
security story.
([RFC 9421 sections 1.4 and 3](https://www.rfc-editor.org/rfc/rfc9421.html#section-1.4))

DPoP is narrower still: its proof covers the HTTP method and URI but not the
message body or general request headers, and a valid DPoP proof is not sufficient
by itself for access control.
([RFC 9449 sections 4 and 11.7](https://www.rfc-editor.org/rfc/rfc9449.html#section-11.7))

**Boundary.** StoryOS can make a request fresh, non-replayable, scope-bound,
and byte-exact. Those properties answer “which authenticated channel/key sent
which bytes?” A compromised first-party page can use the legitimate session,
obtain a legitimate command-bound anti-forgery challenge, and submit a
malicious command. Neither Origin nor an exact digest says that the author chose
the command.

**StoryOS implication, not a decision.** The existing `Client Session Binding`,
anti-forgery nonce, command digest, idempotency, exact retry, expected Heads,
and Core settlement rules remain necessary. They are independent admission
evidence and must not be used as a substitute definition of Author Intent.

### 3. WebCrypto signs caller-selected data, not human intent

**External facts.** WebCrypto exposes signing operations to the page's
JavaScript execution environment. Its security considerations treat script
injection as equivalent to remote code execution and warn that injected script
can use shared key references to create fraudulent messages. The specification
also imposes no general guarantee that key material is hardware-backed.
([Web Cryptography Level 2, security considerations](https://www.w3.org/TR/webcrypto-2/#security-considerations))

**Boundary.** A non-extractable `CryptoKey` can stop raw key export while still
allowing compromised script to invoke `sign()` on attacker-selected command
bytes. A valid signature proves use of that key. It does not prove user
presence, verification, trusted display, or consent.

### 4. Plain WebAuthn can bind a credential ceremony to a challenge, not display an arbitrary command

**External facts.**

1. A WebAuthn authentication assertion signs authenticator data together with
   the hash of `clientDataJSON`. The client data contains the ceremony type,
   server challenge, Origin, cross-origin state, and, where applicable, top
   Origin. Verification checks the expected challenge, Origin, RP ID hash,
   user-presence flag, optional user-verification flag, and signature.
   ([WebAuthn Level 3, verifying an authentication assertion](https://www.w3.org/TR/webauthn-3/#sctn-verifying-assertion))
2. The `UP` flag means the authenticator performed a simple presence test,
   typically a touch. It is explicitly not user verification. `UV` means the
   authenticator locally authorized key use via a PIN, biometric, or similar
   method. WebAuthn says UV does not concretely identify a natural person:
   multiple people can share an authenticator, PIN, or enrolled biometrics.
   ([WebAuthn Level 3, test of user presence](https://www.w3.org/TR/webauthn-3/#test-of-user-presence);
   [user verification](https://www.w3.org/TR/webauthn-3/#user-verification))
3. The authorization gesture consents to the WebAuthn ceremony. The
   authentication prompt selects a credential and may be shown by the
   authenticator or user agent; the general assertion algorithm does not
   require it to render an arbitrary RP command.
   ([WebAuthn Level 3, authorization gesture](https://www.w3.org/TR/webauthn-3/#authorization-gesture);
   [authenticator assertion operation](https://www.w3.org/TR/webauthn-3/#sctn-op-get-assertion))
4. CTAP passes the authenticator an RP ID, `clientDataHash`, credential list,
   extensions, and `up`/`uv` options. The hash cryptographically covers client
   data, but the ordinary authenticator input has no general field requiring a
   human-readable command to be displayed.
   ([FIDO CTAP 2.2, `authenticatorGetAssertion`](https://fidoalliance.org/specs/fido-v2.2-ps-20250714/fido-client-to-authenticator-protocol-v2.2-ps-20250714.html#authenticatorGetAssertion))
5. WebAuthn's security considerations state that malicious code executing on
   an Origin within an RP credential's scope can invalidate any and all
   WebAuthn security guarantees.
   ([WebAuthn Level 3, code injection attacks](https://www.w3.org/TR/webauthn-3/#sctn-code-injection))

**Boundary.** A Server can mint a unique challenge whose server-side record
binds one canonical StoryOS command digest, then require `UV` and verify the
returned assertion. This proves that the registered credential's user verified
an assertion for that RP/Origin and exact server challenge. It does not prove
that a trusted surface displayed the StoryOS command or that the user approved
its semantics. A compromised page can request a challenge for substituted
command bytes and present the user with the ordinary credential ceremony.

The W3C Secure Payment Confirmation specification states this limit directly:
putting payment data in a plain WebAuthn challenge has no specified UX that
shows that data to the user. It treats the challenge as a replay defense, not a
transaction-display field.
([Secure Payment Confirmation, plain-WebAuthn limitations](https://www.w3.org/TR/secure-payment-confirmation/#sctn-intro))

**Honest narrower claim.** Subject to secure credential enrollment and
verification policy, StoryOS could call this a fresh, user-verified WebAuthn
ceremony bound to a command challenge. It cannot call the ceremony proof that
the physical Project Author saw and authorized the exact command under the
compromised-page threat model.

### 5. WebAuthn attestation is credential provenance, not per-command intent

**External facts.** WebAuthn attestation statements concern the public-key
credential and authenticator that created it. Their assurance depends on
attestation type, format, authenticator characteristics, acceptable trust
anchors, and RP policy. Self or no attestation supplies no authenticator
provenance guarantee. Attestation is processed during registration; later
authentication uses the credential assertion.
([WebAuthn Level 3, attestation](https://www.w3.org/TR/webauthn-3/#sctn-attestation))

WebAuthn also warns that attestation alone cannot prove that the attestation
object came from the authenticator the user intended; injected RP script can
participate in substitution attacks.
([WebAuthn Level 3, attestation security considerations](https://www.w3.org/TR/webauthn-3/#sctn-attestation-security-considerations))

**Boundary.** Authenticator attestation may support a policy such as “this
credential key was generated by an accepted authenticator class.” It is not a
per-command trusted display, UP/UV event, or Author Intent record. The word
“attestation” must not collapse these different statements.

### 6. Secure Payment Confirmation shows what browser transaction confirmation adds

**External facts.** Secure Payment Confirmation (SPC) adds payment-specific
data structures and a browser-controlled transaction-confirmation UX to
WebAuthn. The user agent must communicate defined fields such as payee, amount,
and instrument, collect consent, and include the displayed payment data in
signed client data. The specification says this prevents a malicious website
or injected JavaScript from bypassing the payment-specific display.
([SPC introduction](https://www.w3.org/TR/secure-payment-confirmation/#sctn-intro);
[transaction confirmation UX](https://www.w3.org/TR/secure-payment-confirmation/#sctn-transaction-confirmation-ux);
[assertion verification](https://www.w3.org/TR/secure-payment-confirmation/#sctn-verifying-assertion))

**Boundary.** SPC is a payment method and its signed schema is payment-specific.
It is evidence that a browser-owned display-plus-signature design can bind
human confirmation to displayed semantic data. It is not a general Web API for
confirming StoryOS commands and must not be disguised as one.

### 7. Android Protected Confirmation provides a native trusted-display example

**External facts.** Android Protected Confirmation is designed so trusted
firmware protects display integrity, confirmation input, and their atomicity
even if the Android kernel is compromised. A supported device shows a Trusted
UI prompt and returns a confirmation token only after confirmation.
([AOSP Protected Confirmation implementation](https://source.android.com/docs/security/features/protected-confirmation/implementation))

The relying party verifies the signing-key attestation, signature, prompt text,
fresh extra-data nonce, and replay state. Android stresses that only
`promptText` was actually confirmed: undisplayed `extraData` must never be
assumed equivalent to what the user saw. A key configured to require trusted
confirmation can sign only the returned confirmed-data structure.
([Android Protected Confirmation guide](https://developer.android.com/privacy-and-security/security-android-protected-confirmation))

**Boundary.** This is the strongest surveyed directly relevant model for
command confirmation, but it requires a native Android API, supported trusted
hardware/firmware, a human-readable bounded prompt, key attestation, and
server-side verification. It is not available to a portable pure Web Client.
It also proves why hiding a StoryOS digest or command in nondisplayed metadata
is insufficient.

### 8. App/device integrity is useful evidence but still not intent

**External facts.** Apple App Attest can attest a key associated with a
legitimate app instance and later sign assertions over app-supplied
`clientData`, challenge, App ID, and monotonic counter.
([Apple, Validating apps that connect to your server](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server))
Google Play Integrity can return app/device integrity verdicts and bind a
verdict to an app-supplied request hash, with replay defenses.
([Google Play Integrity, standard requests](https://developer.android.com/google/play/integrity/standard))
Android key attestation establishes hardware-backed key properties under a
verified certificate chain.
([Android key attestation](https://developer.android.com/privacy-and-security/security-key-attestation))
Apple LocalAuthentication evaluates device-owner authentication or access
control and returns success/failure. The explanation in its system dialog is an
app-provided `localizedReason`.
([Apple LocalAuthentication overview](https://developer.apple.com/documentation/localauthentication);
[evaluating a policy](https://developer.apple.com/documentation/localauthentication/lacontext/evaluatepolicy%28_%3Alocalizedreason%3Areply%3A%29))

**Boundary.** These mechanisms can raise confidence in app binary, device, key,
and request provenance. Their documented request-hash or assertion flows do
not say that trusted UI displayed the request or that a human confirmed it.
LocalAuthentication proves successful device-user policy evaluation, but its
app-supplied reason is not documented as a signed exact-transaction
confirmation. These mechanisms can complement a trusted confirmation
implementation but cannot replace one.

### 9. A native bridge is transport until the native side enforces a trust boundary

**External facts.** Chrome native messaging is an extension API: an extension
with `nativeMessaging` permission can exchange JSON with a registered native
host whose manifest allowlists extension Origins. The API is unavailable in
content scripts; a content script must relay through an extension page or
service worker.
([Chrome native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging))

**Boundary.** The bridge can establish which installed extension reached which
registered native host. It does not define command semantics, user
authentication, trusted display, confirmation, signing, freshness, or
settlement. If an extension or native host blindly signs page-provided data,
the compromised page still chooses the command.

A bridge becomes relevant to strong Author Intent only if the trusted
extension/native endpoint treats all page messages as untrusted, independently
obtains or validates the server challenge and canonical command, displays the
meaningful command outside page control, collects explicit confirmation, binds
the displayed content and fresh challenge to protected-key output, and returns
only that bounded evidence. Those requirements are an inference from the
source boundaries; no surveyed browser native-messaging API supplies them
automatically.

## Capability matrix for the accepted attacker model

| Mechanism | Server-verifiable | Binds exact command/digest | Human-presence or verification evidence | Trusted semantic display | Survives compromised first-party page as author-intent proof |
| --- | --- | --- | --- | --- | --- |
| `isTrusted` event / user activation | No standard remote proof | No | recent interaction only | No | No |
| Session + Origin/Host + CSRF nonce | Yes | Yes, if StoryOS binds it | No | No | No |
| WebCrypto / HTTP Message Signature | Yes | Yes, if covered | No | No | No when the page can invoke the key |
| DPoP | Yes | Method/URI, not body | No | No | No |
| Plain WebAuthn assertion with `UP` | Yes | Challenge or server mapping | presence only | RP/credential ceremony, not command | No |
| Plain WebAuthn assertion with required `UV` | Yes | Challenge or server mapping | authenticator user verified | RP/credential ceremony, not command | No |
| WebAuthn authenticator attestation | Registration-time | Credential creation client data | registration ceremony only | Not per command | No |
| App/device/key integrity attestation | Yes | Often request hash/data | Not inherently | No | No |
| SPC | Yes | Signed payment-specific data | WebAuthn UP/UV | Yes, payment fields | Yes for the specified payment ceremony; not usable for StoryOS |
| Android Protected Confirmation | Yes | Confirmed structure plus checked nonce/data | explicit trusted confirmation | Yes, bounded prompt | Yes under supported native/hardware assumptions |
| Native bridge alone | Channel-specific | Only if separately designed | No | No | No |
| Trusted native/extension confirmation mediator | Potentially | Only if separately designed | Only if separately designed | Only if independently rendered | Potentially; this is a StoryOS decision, not a Web primitive |

## Claims a pure Web implementation may and may not make

### Defensible, if the named checks actually pass

- “A trusted browser input event activated this browsing context recently.”
- “Browser integration normalized this observed input sequence into one command
  in the tested non-compromised implementation.”
- “The Server derived this User and exact Project Scope from the current Client
  Session Binding.”
- “The accepted request matched this method, route, schema, canonical command
  digest, idempotency record, nonce, and expected Heads.”
- “This registered WebAuthn credential produced a valid assertion for this RP,
  Origin, and challenge with `UP`, and with `UV` when required.”
- “This key signed these covered bytes.”
- “Core durably settled this exact command attempt with this Receipt.”

### Not defensible under a compromised first-party page

- “The trusted event authorized the command the page later submitted.”
- “`isTrusted` means the event payload or derived DTO is trustworthy.”
- “WebAuthn `UP` proves the Project Author acted.”
- “WebAuthn `UV` proves a uniquely identified natural person approved the
  command.”
- “The WebAuthn challenge or signed digest was shown to or understood by the
  user.”
- “Authenticator attestation proves per-command author intent.”
- “A non-extractable WebCrypto key means compromised script cannot sign.”
- “Origin, SameSite, CORS, CSRF, TLS, DPoP, a digest, or an idempotency key
  proves author choice.”
- “A native bridge is trusted confirmation.”
- “A hidden digest or `extraData` field is equivalent to confirmed visible
  semantics.”

## StoryOS-local contract pressure

These are research-derived compatibility observations, not resolutions:

1. `CONTEXT.md` currently defines **Author Intent** as immutable
   “Host-attested” evidence that one explicit interaction authorized one exact
   author-owned command, and says an untrusted client cannot assert it.
2. The threat model explicitly treats first-party Client state as potentially
   compromised and still unable to attest Author Intent.
3. The protocol correctly separates Client Session Binding, anti-forgery,
   command digest, idempotency, and Author Intent.
4. The first-slice contract correctly says the Web Client cannot attest author
   intent.
5. The editor verification gate supplies browser-observed interaction and
   cardinality evidence that one physical input maps to at most one command in
   the implementation under test; it does not supply a
   compromised-client-resistant authorization oracle.

The first four boundaries are mutually consistent only if “Host” names a
component outside the compromised Web page that can obtain trustworthy
command-specific confirmation, or if the Author Intent security claim is
explicitly narrowed for a stated class of direct Web edits. Calling page event
code, a session nonce, or plain WebAuthn the Host would preserve the vocabulary
while violating the stated attacker model.

## Newly sharp question for the downstream decision

Must **every** direct manual editor command remain author-intent-secure when the
entire first-party page is compromised?

- If yes, which trusted, independently rendered confirmation/editor surface is
  in the delivery boundary, and how can normal discovery writing remain usable
  when every exact edit needs that surface?
- If no, what strictly named lower assurance is accepted for direct Web edits,
  which command classes still require strong trusted confirmation, and which
  current “Host-attested exact command” clauses are explicitly superseded?

Primary sources cannot choose that product/security boundary. It belongs only
to
[Specify Trustworthy Author Intent Admission](https://github.com/FrankQDWang/StoryOS/issues/68).

## Primary source set

- [WHATWG HTML: Tracking user activation](https://html.spec.whatwg.org/multipage/interaction.html#tracking-user-activation)
- [W3C Web Authentication Level 3](https://www.w3.org/TR/webauthn-3/)
- [FIDO Client to Authenticator Protocol 2.2](https://fidoalliance.org/specs/fido-v2.2-ps-20250714/fido-client-to-authenticator-protocol-v2.2-ps-20250714.html)
- [W3C Secure Payment Confirmation](https://www.w3.org/TR/secure-payment-confirmation/)
- [W3C Web Cryptography Level 2](https://www.w3.org/TR/webcrypto-2/)
- [RFC 9421: HTTP Message Signatures](https://www.rfc-editor.org/rfc/rfc9421.html)
- [RFC 9530: Digest Fields](https://www.rfc-editor.org/rfc/rfc9530.html)
- [RFC 9449: OAuth 2.0 Demonstrating Proof of Possession](https://www.rfc-editor.org/rfc/rfc9449.html)
- [Android Protected Confirmation](https://developer.android.com/privacy-and-security/security-android-protected-confirmation)
- [AOSP Protected Confirmation implementation](https://source.android.com/docs/security/features/protected-confirmation/implementation)
- [Apple App Attest server validation](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server)
- [Apple LocalAuthentication](https://developer.apple.com/documentation/localauthentication)
- [Google Play Integrity standard requests](https://developer.android.com/google/play/integrity/standard)
- [Android key attestation](https://developer.android.com/privacy-and-security/security-key-attestation)
- [Chrome native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)

All sources were accessed on 2026-07-23.
