# Run the Local Real Model Route

The real Volcengine Agent Plan route runs only on the author's Mac local deployment (ADR 0039, ADR 0048). The Server offers the destination, and the separate `storyos-worker` resolves the Keychain credential. The Server never reads the credential.

## One-time setup

Only the author does these steps. No agent reads, prints, or stores the API key.

1. Keep the API key in the login keychain as the generic password item of ADR 0048: service `storyos-volcengine-agent-plan`, account `frankqdwang`.
2. Make a local code signing identity. In Keychain Access, select **Certificate Assistant > Create a Certificate**. Enter the name `StoryOS Local Development`, select the identity type **Self Signed Root** and the certificate type **Code Signing**, and create it in the login keychain.
3. Run `scripts/real-route-worker.sh --check` one time. The script signs a copy of the packaged Worker at `target/local-real-route/storyos-worker`. The first signature can ask for access to the signing key. Select **Always Allow**.
4. In Keychain Access, open the API key item, select **Access Control**, add `target/local-real-route/storyos-worker` with **+**, and save. macOS asks for the login password one time.

Each later rebuild is signed with the same identity and the identifier `dev.storyos.worker`. Thus the access of step 4 stays valid, and no prompt occurs.

## Run

1. Run `make release-package`.
2. Start the Server with `STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, and `STORYOS_MODEL_CREDENTIAL_REFERENCE=macos-keychain:storyos-volcengine-agent-plan/frankqdwang`. For the runtime qualification only, also set `STORYOS_MODEL_QUALIFICATION_AUTHORIZATION` to the link of the author authorization (ADR 0049).
3. Start the Worker with `scripts/real-route-worker.sh`. Give it the same `STORYOS_DATABASE_URL`. Add `--once` to settle one claim and stop.

The script stops with a message when the signing identity is missing. It never runs an unsigned Worker.

The Worker never shows a Keychain prompt. When the item needs a prompt, or when a read holds for more than 10 seconds, the request refuses before the dispatch claim with `destination_credential_unavailable`. Then do step 4 of the setup again.
