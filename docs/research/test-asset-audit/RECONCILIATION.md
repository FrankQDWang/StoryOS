# Inventory and coverage reconciliation

All 256 test files have verdict tables: 865 runtime cases. The inventory stores attribute/leading-whitespace starts in some files; verdict tables use declaration lines. Two dynamic Node files instead use each literal case location: protocol-boot has five blocked inputs plus one success; required-global-teardown has three parameter inputs. These explain the declaration-set differences, not missing tests. All 798 verdict source locations are represented by those declarations or explicit cases.

No Web support file remains unreviewed. The report generator was accidentally invoked once with two group names; its second argument expects JSON, so it made no changes. Both groups were regenerated separately and checked before this checkpoint.

## Accepted contracts versus unreachable inputs

- AD032 changes from DELETE D5 to KEEP K1. Migration 0002 explicitly stores generation as numeric(20,0), bounded to the full u64 range. Signed narrowing is a realistic storage conversion regression on accepted data. Bootstrap generation 1 is not a change to that persisted contract; ordinary low-value cases do not cover it.
- Other D5 rows distinguish a current producer guarantee from external authorization. Core missing-Project inputs follow a failed scoped lookup; wrong local command-kind/digest tuples cannot be independently supplied through current Server constructors. Existing HTTP tests own actual changed-request/stored-Challenge refusal. No internal guard-only mutation kill is claimed.
- Server origin configuration is validated before binding construction. The expiry portion of SV028 has real Web-host coverage; independently changed binding identity/generation remains a synthetic config tuple at this baseline. ADR 0013 requires exact accepted identities but does not create a current rotation producer. Revisit if a session rotation/config loading path is implemented.
- Contracts D5 cases mutate private historical constant structures or pinned Git-object copies. They do not consume live tracker inputs. Full artifact comparison protects emitted facts, not every redundant private verifier guard.
- D5 conclusions retain their source-specific limitations and enter the same random population as other DELETE rows. A selected surviving guard mutation is a MISS, never a substitute sample or a claimed product kill.

## Coverage chains and execution prerequisites

References to deleted shared replay tests now point directly to retained Create Volume:760. Application outcome points to public Project:253; static profile points to retained Project open:31. Their earlier intermediate tests are not required deletion owners.

The following DELETE rows still cite a current MERGE/MOVE owner. They can be removed while that owner remains. To also remove or move the owner, first complete its named assertion transfer and verify its receiving test. Until then, the receiving test is not claimed to provide equivalent coverage. This is an explicit thinning-plan dependency, not immediate permission to remove all rows together.

| DELETE row | Required owner transfer before owner removal |
|---|---|
| CO003 | NP030 |
| CO004 | NP030 |
| CO005 | NP030 |
| CO006 | NP030 |
| CO007 | NP025 |
| CO008 | NP025 |
| CO009 | NP025 |
| CO011 | NP025 |
| CO058 | BD021 |
| CO082 | NP083 |
| CO085 | NP135 |
| CO134 | NP128 |
| CO142 | NP135, NP111 |
| CO143 | NP111 |
| CO158 | NP104 |
| CO160 | NP104 |
| CO172 | NP129 |
| CO183 | BD017 |
| AP003 | NP025 |
| AP004 | NP025 |
| AP005 | NP025 |
| AP006 | NP025 |
| AP007 | NP030 |
| AP008 | NP030 |
| AP009 | NP030 |
| AP010 | NP030 |
| AP013 | BD021 |
| AP014 | BD021 |
| AD098 | NP138 |
| SV026 | NP006 |
| SV035 | NP006 |
| CT012 | NP138 |
| CT016 | NP006 |
| NP001 | BD003 |
| BD035 | NP129, NP135 |

All current coverage paths exist. Source comparison does not replace the pending random mutation self-check.
