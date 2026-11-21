## Plan of the second part of the live run (S10-08): R3b and R4

The first part ([plan](https://github.com/FrankQDWang/StoryOS/issues/1774#issuecomment-7061417077)) stopped by its rule at R3. The coordinator decided the next steps ([decision](https://github.com/FrankQDWang/StoryOS/issues/1774#issuecomment-7061543995)): R1, R2, and R3 stay as evidence as they occurred, request 3 runs one more time as R3b, and R4 runs as the first plan gave it. The run then has 5 AgentRuns, and the ticket permits 6.

- **Head, package, route, topology, database, and script:** as in the first part. Head `6e8b97a48f76747cc8086c0bff3108a728e5d32e`, the same release package (manifest `sha256:4b668f4b10cad52eb2b42228cfa1fe5ee7c902fb3da014e32be5814f24d496f9`), and a new `scripts/dev-postgres.sh` database. The script is the script of the first part with only the Projects 归来 C and 夜渡 D and the numbers below.
- **Rehearsal:** the script on `host_fake` on this head passed: 2 Runs `completed`, 4 fake requests, no refusal.

### The two author requests

| # | Project | Author request | What the request shows |
| --- | --- | --- | --- |
| R3b | 归来 C, the same fixed paragraph | 请改写这一段，保留其中的情节。, with no selected Skill | Request 3 again, for one complete Run of that request: F1, F2, and whether the model loads a Skill from the catalog. The record gives the candidates of R3 and R3b for each measure, and it selects neither. |
| R4 | 夜渡 D, the Chapter and the Project Instruction of Run C of S10-04 | 请修改这一章。, with the selected Skill `local-revision` | As in the first plan. |

### Planned numbers

The planned number of a Run is the number of the stop rule. Before each Run, the script sets the cap of the request guard to the planned number of that Run. The script stops the run when a Run does not end 120 seconds after its planned number of requests, or when a number of tokens is above its planned number. Each planned number of requests has one step more than the known steps of the Run.

| Run | Known steps that the number counts | Planned number of model requests | Planned number of reported input tokens |
| --- | --- | --- | --- |
| R3b | `load_skill`, `propose_edit`, `finish`, the final answer, and one more step | 5 | 30,000 |
| R4 | `propose_edit`, `finish`, the final answer, and one more step | 4 | 40,000 |
| **Both Runs** | | **9** | **70,000** |

For information only: the three requests of R3 had 17,215 reported input tokens, and the three requests of Run C of S10-04 had 24,357.

### Other stops and evidence

The other stops are those of the first plan. The evidence of this part goes on the same tag `evidence/1774-live-writing-rules-findings-2026-11-21`, in its own directory, and the record has both parts.
