---
status: accepted
---

# Read the Local Agent Plan Key from the Ignored Env File

On 2026-10-11, during [S4-02b: Admit an Authorized Runtime Qualification of the Agent Plan Route](https://github.com/FrankQDWang/StoryOS/issues/1096), the author accepted this decision. On the author's Mac local deployment, the Agent Plan API key is in the git-ignored `.env` file of the main checkout. A Credential Reference of the form `env:STORYOS_<NAME>` names it. Only the Worker process gets the value.

## Context

[ADR 0048](0048-use-the-deployment-model-destination-by-default.md) puts the operator-quota key in a macOS Keychain generic password, and [ADR 0039](0039-open-the-model-gateway-seam-between-committed-dispatch-boundaries.md) permits environment variables only for development and tests. The first live qualification run showed that macOS binds Keychain access to the signature of the binary. Each rebuild of `storyos-worker` asks for the login password again. While that prompt was open, the Keychain read held the Worker for 24,707 seconds. The author rejected the password prompts and a local code signing setup as too much friction for development.

The Mac local deployment is the development deployment of the author. It is also the deployment of the Stage 4 runtime qualification and the real-author journey (ADR 0039). The key is a personal Agent Plan key, and the author controls its quota in the Volcengine console.

## Decision

- A Credential Reference can have the form `env:STORYOS_<NAME>`. Only a variable name that starts with `STORYOS_` and has only uppercase letters, digits, and underscores is a reference. The local real route uses `env:STORYOS_AGENT_PLAN_API_KEY`.
- The value is in the `.env` file of the main checkout. Git ignores this file, and it has mode 600. The committed `.env.example` lists the variables with placeholder values only.
- `scripts/real-route-worker.sh` loads the `STORYOS_` variables of that file into the Worker process only, and then runs the Worker. It does not change the shell that starts it, and it prints no value.
- Records, Wire Payload Projections, diagnostics, problems, and errors keep only the reference, never the value. Test fixtures use only fake values. The Worker reads the value during preparation, into the single-use prepared value. The Server never reads it.
- The key is never exported in a shell profile, because each agent shell loads the profile of the author. Agents never read, print, or search `.env`. An agent can only make sure that a variable name is present.
- The Keychain reference form of ADR 0048 stays valid. The Keychain resolver never shows a prompt. An item that needs a prompt is unavailable, and so is a read that holds for more than 10 seconds. Then the request refuses before the dispatch claim.

## Considered options

- A Keychain item with access for each application was rejected. Then any process of the user can read the key without a prompt, and the author must still change the item.
- A local code signing identity for the Worker was rejected. It needs a certificate and a signing step, and the author rejected that friction.
- A key exported in the shell profile was rejected. Then each agent shell, each test process, and each environment dump holds the key.
- A Keychain read in the launcher script with the Apple `security` tool was rejected for this deployment. The author selected the standard `.env` practice.

## Consequences

- The Worker process environment holds the key while the Worker runs. The Worker starts no child process. In Rust 2024, the removal of an environment variable is an unsafe operation. The workspace forbids unsafe code, so the Worker does not remove the variable.
- A process that can read the files or the process environment of the author can read the key. This is acceptable for the personal Stage 4 deployment.
- The credential canary covers the environment form.
- This decision applies only to the author's Mac local deployment. The Linux VPS real route stays unavailable until a later ADR selects its resolver (ADR 0039). A move off Agent Plan for commercial use needs its own ADR (ADR 0048).
