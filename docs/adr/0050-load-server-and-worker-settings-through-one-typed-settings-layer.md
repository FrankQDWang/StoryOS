---
status: accepted
---

# Load Server and Worker Settings Through One Typed Settings Layer

On 2026-10-11 the author accepted this decision for [Load Server and Worker Settings Through One Typed Settings Layer and Remove the Keychain Path](https://github.com/FrankQDWang/StoryOS/issues/1099). The Server and the Worker load their settings through `figment` into typed structs. Each secret value is a `secrecy::SecretString`. For a real-route run, `dotenvy` reads the git-ignored `.env` file. The macOS Keychain credential path is removed.

## Context

At `main` `cf8c2d11`, the Server and the Worker read their configuration with hand-written `std::env::var` calls. No typed model or secret type held the challenge secret, the database URLs, or a credential. A `Debug` print or an error text can show them.

S4-01 put the Agent Plan key in a macOS Keychain item and resolved it in the Worker ([ADR 0048](0048-use-the-deployment-model-destination-by-default.md)). macOS binds Keychain access to the signature of the binary, so each rebuild asked for the login password. One read held the Worker for 24,707 seconds while the prompt was open. The author rejected the prompts and a local code signing setup, and selected the standard `.env` practice.

Development runs in git worktrees. A worktree has no copy of the git-ignored `.env` file, but a real-route run must work from each worktree.

## Decision

### One typed settings layer

- The `storyos-settings` crate loads the `STORYOS_` variables into the typed settings of each binary. It takes only the variables that the binary declares. Thus the Server never takes `STORYOS_AGENT_PLAN_API_KEY`.
- Each value enters the settings as text. A setting that is a number is parsed and validated at startup.
- Each secret is a `SecretString`: `STORYOS_CHALLENGE_SECRET`, `STORYOS_DATABASE_URL`, and `STORYOS_AGENT_PLAN_API_KEY`. A secret leaves the type only at the place that uses it.
- A wrong or missing setting stops startup with a message that names the variable and the env file that the settings read. The message never holds a value. The Server requires `STORYOS_CHALLENGE_SECRET` with at least 32 bytes before it starts.
- Test-only hold variables stay outside the settings.

### The env file of a real-route run

- Only the Worker reads an env file. The Server reads its settings from its process environment only, so it never reads the key, not even as a line of the file.
- The Worker reads an env file only in a run that needs the key. That is a run with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, or a run with an explicit `STORYOS_ENV_FILE`. Both values come from the process environment. A test, a verification command, and a fake-destination run read no env file. Thus none of them can get the key or send a live request by accident.
- The lookup order has three steps. First, the path in `STORYOS_ENV_FILE`. Second, the `.env` of the current checkout. Third, the `.env` of the main checkout, in the parent directory of `git rev-parse --git-common-dir`. Nothing copies or links `.env` into a worktree.
- `dotenvy` reads the file into the settings, and the settings layer drops each line that the Worker does not declare as it reads it. It does not change the process environment. A process variable takes precedence over the same variable in the file.
- The `.env` file has mode 600, and git ignores it. The committed `.env.example` lists each product variable with a placeholder value only. The key is never exported in a shell profile, because each agent shell loads the profile of the author. Agents never read, print, or search `.env`.

### The credential of the Agent Plan route

- The Credential Reference of the operator-quota key is `settings:STORYOS_AGENT_PLAN_API_KEY`. The binding records keep only this reference (ADR 0048).
- Only the Worker takes the key. The Worker resolves the reference from its settings into the single-use prepared value (ADR 0039). An absent key refuses each request before the dispatch claim with `destination_credential_unavailable`. A real-route Worker without the key does not start.
- The `STORYOS_MODEL_CREDENTIAL_REFERENCE` variable, the Keychain resolver, the `security-framework` dependency, the environment resolver, and the launcher script are removed.

### Superseded statements

This record supersedes the Keychain statements of [ADR 0004](0004-adopt-postgresql-service-and-project-isolation-boundary.md), [ADR 0039](0039-open-the-model-gateway-seam-between-committed-dispatch-boundaries.md), and ADR 0048. The local real route stays on the author's Mac deployment. The Linux VPS real route stays unavailable until a later ADR selects its secret source (ADR 0039).

## Considered options

- Keep the Keychain item, with access for each application. Rejected: any process of the user can then read the key without a prompt, and the author must still change the item.
- Sign the Worker with a fixed local code signing identity. Rejected: it needs a certificate and a signing step, and the author rejected that friction.
- Export the key in the shell profile. Rejected: each agent shell, each test process, and each environment dump then holds the key.
- Load `.env` into the process environment in each run. Rejected: verification runs from a checkout with `.env` then get the real destination and the key.

## Consequences

- The workspace gets direct `figment`, `secrecy`, and `dotenvy` dependencies. `storyos-application` uses `secrecy` for the resolved credential.
- A process that can read the files of the author can read the key. This is acceptable for the personal Stage 4 deployment, and the author controls the quota.
- The credential canary covers the settings form.
- A move off Agent Plan for commercial use needs its own ADR (ADR 0048).
