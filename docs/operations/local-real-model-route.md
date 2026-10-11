# Run the Local Real Model Route

The real Volcengine Agent Plan route runs only on the author's Mac local deployment (ADR 0039, ADR 0048). The Server offers the destination, and the separate `storyos-worker` resolves the credential. The Server never reads the credential value.

## Credential

On this deployment the API key is in the git-ignored `.env` file of the main checkout (ADR 0050). The Credential Reference is `env:STORYOS_AGENT_PLAN_API_KEY`. Records keep only this reference, never the value.

Only the author does these steps:

1. Copy `.env.example` to `.env` in the main checkout, and set mode 600 with `chmod 600 .env`.
2. Put the key in the `STORYOS_AGENT_PLAN_API_KEY` line.

Do not export the key in `~/.zshrc` or in another shell profile, because each agent shell loads that profile. Do not commit `.env`. Agents never read, print, or search `.env`. An agent can only make sure that a variable name is present.

The Keychain reference `macos-keychain:storyos-volcengine-agent-plan/frankqdwang` stays valid. The Worker never shows a Keychain prompt. When the item needs a prompt, or when a read holds for more than 10 seconds, the request refuses before the dispatch claim.

## Run

1. Run `make release-package`.
2. Start the Server with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_MODEL_CREDENTIAL_REFERENCE=env:STORYOS_AGENT_PLAN_API_KEY`. For the runtime qualification only, also set `STORYOS_MODEL_QUALIFICATION_AUTHORIZATION` to the link of the author authorization (ADR 0049). The Server does not need the key.
3. Start the Worker with `scripts/real-route-worker.sh`, and give it `STORYOS_DATABASE_URL`. Add `--once` to settle one claim and stop.

The script loads the `STORYOS_` variables of `.env` into the Worker process only. It does not change the shell, and it prints no value. It stops when `.env` is missing or does not have mode 600. A worktree has no copy of `.env`, so the script reads the `.env` of the main checkout.

When the variable is absent or empty, each Agent Plan request refuses before the dispatch claim with `destination_credential_unavailable`.
