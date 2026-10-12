# Run the Local Real Model Route

The real Volcengine Agent Plan route runs only on the author's Mac local deployment (ADR 0039, ADR 0048). The Server offers the destination, and the separate `storyos-worker` resolves the credential. The Server never reads the env file or the key.

## Settings and the key

The Server and the Worker load their `STORYOS_` settings through one typed settings layer (ADR 0050). The Agent Plan key is the `STORYOS_AGENT_PLAN_API_KEY` setting of the Worker. Its Credential Reference is `settings:STORYOS_AGENT_PLAN_API_KEY`. Records keep only this reference, never the value.

Only the author does these steps:

1. Copy `.env.example` to `.env` in the main checkout, and set mode 600 with `chmod 600 .env`.
2. Put the key in the `STORYOS_AGENT_PLAN_API_KEY` line. Other lines are optional.

Do not export the key in `~/.zshrc` or in another shell profile, because each agent shell loads that profile. Do not commit `.env`, and do not copy or link it into a worktree. Agents never read, print, or search `.env`.

## Which runs read the env file

Only the Worker reads an env file. The Server reads its settings from its process environment only. The Worker reads an env file only in a run with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` in its process environment, or with `STORYOS_ENV_FILE=<path>`. Tests, verification commands, and fake-destination runs read no env file.

The lookup order is:

1. The path in `STORYOS_ENV_FILE`, if the process environment has it.
2. The `.env` of the current checkout, if it exists.
3. The `.env` of the main checkout: the parent directory of `git rev-parse --git-common-dir`.

Thus a real-route run works from each worktree. A process variable takes precedence over the same line of the file. The file does not change the process environment.

## Run

1. Run `make release-package`.
2. Start the Server with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and its other settings in the process environment. `STORYOS_CHALLENGE_SECRET` must have at least 32 bytes. For the runtime qualification only, also set `STORYOS_MODEL_QUALIFICATION_AUTHORIZATION` to the link of the author authorization (ADR 0049).
3. Start `target/release-package/storyos-worker` with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` and `STORYOS_DATABASE_URL`. Add `--once` to settle one claim and stop.

A real-route Worker without the key does not start. Its message names `STORYOS_AGENT_PLAN_API_KEY` and the env file that it read, never a value. A wrong setting stops each binary in the same way.
