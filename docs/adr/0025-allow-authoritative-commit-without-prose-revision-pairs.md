# Allow an Authoritative Commit Without Prose Revision Pairs

A Manuscript Structure Transition changes Authoritative State and must allocate one Authoritative Commit. Fabricating a prose Revision so the Commit can keep a required revision pair was rejected. Omitting the Commit was rejected because Authoritative State includes manuscript structure. The Commit may have empty revision pairs when no Authoritative Revision changed. It names the transition by prior and resulting Manuscript Tree Revision, the affected Volume or Chapter identities, and only the Revision pairs that actually occurred.

## Considered options

- Inventing a structure Revision object to satisfy the old Commit shape was rejected. That object would not be an Authoritative Revision of prose and would falsify Revision Lineage.
- Recording only an Author Action and Snapshot was rejected. The Commit sequence is the authority-changing clock; structure changes would disappear from it.
- Binding only Admission, Receipt, and Activity was rejected. Those records already exist; they do not name the structure head.
- Embedding a full tree image in the Commit was rejected. The Commit names the transition. The Canonical Manuscript Tree remains the current hierarchy, not a second copy inside the Commit.
