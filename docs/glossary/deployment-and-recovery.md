# Glossary: Deployment, export, deletion, and recovery

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Project Export Archive**:
A versioned, self-describing, integrity-protected portable archive of one exact Project Scope, produced from one transactionally consistent boundary and containing every non-secret canonical record and immutable payload exportable at its Pinned Export Source, plus lifecycle, redaction, retention, and provenance facts for any known gap, required to restore that Project without consulting a disposable projection or external runtime. Every entry name is admitted under the manifest's exact versioned Archive Path Profile before sorting or digest coverage, so platform path rules, Unicode normalization, or case behavior cannot reinterpret archive identity. It preserves original User, Project, and object identities; excludes caches, retrieval and embedding projections, secret material, and Provider-held state; and grants neither destination access nor ownership transfer.
_Avoid_: Selected-table dump, backup, cache snapshot, credential bundle, project copy, Worker-time live Project state as archive input

**Pinned Export Source**:
The immutable, Project Scope-bound copy of the exportable canonical facts required by one admitted export operation: manuscript facts for a human-readable export, or the complete exportable families for a Project Export Archive. It is created at admission, bound to that operation's source Snapshot locator, unavailable when that Snapshot is missing or expired or when the source is missing, partial, or digest-invalid, and discarded after settlement; both export Workers read only this source and never live Project state.
_Avoid_: Canonical Query Snapshot as frozen export input, live Project rows, Activity-position reconstruction, Worker-time current state, settled manuscript or ZIP as the source, second Snapshot authority, nested copy of the packing operation's own source

**Project Restore**:
The validating import of one Project Export Archive as the same Project Scope into a target that is authorized for the same durable User identity and does not already contain that Scope. Restore stages and verifies the complete archive, schema compatibility, digests, referential closure, scope, and known lifecycle/redaction gaps before making the Project atomically visible, then deterministically rebuilds disposable projections; any identity conflict, partial archive, unsupported schema, or divergent existing Project fails closed without merge, overwrite, identity remapping, or resurrection of unavailable payload. Unresolvable Credential References remain explicitly Unbound. Creating a copy, fork, new Project identity, or ownership transfer requires a separate future domain contract.
_Avoid_: Import as new, ID remapping, partial merge, overwrite restore, ownership transfer

**Foundation Validation Deployment**:
The initial product stage in which one bootstrapped User uses StoryOS to write a real novel while exercising the same Project Scope and Project Isolation contracts required when more Users are served later. It is a validation stage, not a distinct single-user domain model or permission shortcut. Local development uses a Mac and OrbStack PostgreSQL. Production PostgreSQL is adopted hosted infrastructure. Server, Worker, and Web stay paired on a Linux VPS behind TLS.
_Avoid_: Product-wide single-user mode, global current User, throwaway domain model, implicit Project access, treating the local database host as the production recovery host, Vercel as the production Web host, treating this stage as PASS-CLOUD

**Foundation Validation Public Origin**:
The one Server-held operator-configured `https` first-party Origin and matching DNS Host for the Foundation Validation Deployment; it is a Client Session Binding transport profile, not the later controlled-cloud handoff. Forwarded headers do not define it.
_Avoid_: PASS-CLOUD, EV-CCD, HND-005, Vercel origin, proxied CDN, Cloudflare orange cloud, bind address as the printed origin, X-Forwarded-Host as the allowed site, public IP as Host, controlled-cloud multi-user deployment

**Adopted Hosted Infrastructure**:
A widely validated external PostgreSQL hosting service that production uses instead of an operator-owned database host. Local development may use OrbStack PostgreSQL. The vendor does not become Author, User, Project, or admission authority, and it does not host the Protected Web Client.
_Avoid_: rebuilding a production physical WAL archive, Vercel or another second Web host, Supabase Auth or PostgREST as product surfaces, silent contract override

**Release 1 Storage Activation**:
The inspectable `Active` proof that one exact Release 1 PostgreSQL storage identity—catalog, checksum chain, ledger, and activation record—matches the packaged release and is the only proof that admits Server or Worker traffic.
_Avoid_: Trusted Local Session Bootstrap, Server-startup DDL, schema version as readiness, HTTP health check, sidecar activation file, test SQL apply as the production owner, Recovery Visibility Proof, Release 1 Recovery Chain, adopting an unknown non-empty database

**Release 1 Recovery Chain**:
The StoryOS-owned maintenance boundary that binds one Active Release 1 storage identity to host-loss recovery for that identity. On local OrbStack PostgreSQL the isolated drill binds physical Recovery Copies that StoryOS can read. On hosted production PostgreSQL the vendor holds the daily physical backup; StoryOS does not possess the files and proves only that backups exist and that a non-live restored copy can pass Recovery Visibility Proof. Runtime may observe only an install-once proof. It is not Release 1 Storage Activation.
_Avoid_: storyos-storage, Server or Worker administration, ephemeral verify drill as the live production owner, same-disk VPS backup as the production promise, in-place vendor PITR as release proof, Trusted Local Session Bootstrap, per-request chain-health gate, adopting Supabase Auth as session bootstrap

**Foundation Monorepo**:
The one StoryOS repository that jointly governs the Rust workspace, production Web Client, external-contract source, and checked-in generated contract artifacts so a compatible product change is reviewed and reproducibly verified as one unit. It does not make internal package boundaries an author setting or admit disposable prototypes or `.reference` as production members.
_Avoid_: Split runtime repositories, separately authoritative generated SDK, prototype workspace

**Server/Worker Separation Boundary**:
The modular-monolith boundary in which the Server owns public transport and trusted request admission, Core owns authoritative transitions, and the Worker executes only durably claimed asynchronous or external work through Core-owned contracts. Server and Worker remain independently startable and deployable, while the Foundation default may co-locate them; neither becomes a separate authority store, microservice, or broker-owned workflow.
_Avoid_: HTTP background thread as recovery boundary, mandatory separate service fleet, worker-owned truth

**Prototype Evidence Asset**:
A disposable, bounded-risk experiment retained only to reproduce and inspect the exact question, environment, observations, and limitations that informed an accepted StoryOS contract. It is frozen rather than absorbed into production, and may be deleted only through an explicit reviewable decision after its durable evidence record is sufficient.
_Avoid_: Production seed, evolving pre-production branch, runtime dependency, unrecorded experiment

**Reference Evidence Locator**:
A repository-owned non-runtime record that makes an upstream design or source observation reproducible by naming its canonical location, exact immutable revision or digest, license, retrieval date, and relevant scope. A local `.reference` snapshot is not a Locator merely because it exists and never becomes a production dependency, workspace member, test input, or accepted evidence without this independently reviewable identity.
_Avoid_: Machine-local snapshot, vendored runtime source, implicit dependency, unpinned citation

**Foundation Recovery Service Profile**:
The minimum durability and disaster-recovery promise for the Foundation Validation Deployment. Every author-visible successful commit survives an ordinary process or power crash with zero acknowledged-data loss through synchronous PostgreSQL commit. On hosted production PostgreSQL, loss of the vendor compute host is covered by the vendor daily physical backup; the recovery-point objective is at most twenty-four hours until a later tightening adopts vendor PITR. StoryOS does not possess those backup or WAL files. A restored Project becomes readable only after Recovery Visibility Proof. A local OrbStack drill remains the Hold and proof oracle. This Profile does not require a synchronous replica, automatic failover, or a high-availability cluster.
_Avoid_: Asynchronous author acknowledgement, same-disk backup as the production promise, treating a logical dump as a Recovery Copy, treating fifteen-minute StoryOS-owned WAL as the current hosted promise, untested backup file, Foundation high-availability cluster

**Recovery Copy**:
A bounded PostgreSQL base backup, WAL segment, or equivalent recovery-chain member retained only to meet the Foundation Recovery Service Profile, never as a Project Export, ordinary read source, or independent authority. It remains subject to the exact retained lifecycle ledger and can serve a Project only through a successful Recovery Visibility Proof.
_Avoid_: Export archive, cold Project copy, raw recovery database, alternate source of truth, Release 1 Recovery Chain

**Recovery Visibility Proof**:
The inspectable determination that a restored Project Scope includes and has applied every recoverable later lifecycle decision relevant to the selected recovery target, including Redaction, Tombstone, retention, and availability gaps, before any ordinary read or execution is enabled. A missing or unverifiable lifecycle range fails closed to a recovery hold rather than exposing an older view as current. A contract-valid SQL NULL current Chapter is not a Chapter gap.
_Avoid_: Successful database boot, point-in-time restore alone, best-effort lifecycle replay, counting a lawful empty Project as a missing Chapter

**Empty Project create**:
The public `createProject` command that inserts one Project with `current_chapter_id` as SQL NULL and creates no Volume, Chapter, or manuscript payload. Canonical Snapshot create may already write Replay Generation and Replay Floor; those records are not a Chapter. This committed empty state is lawful.
_Avoid_: Starter Chapter, manufactured manuscript, treating emptiness as missing recovery evidence

**Non-Revival Recovery Oracle**:
The deterministic recovery and replay test rule that compares recovered state against retained historical facts plus current lifecycle availability, not against a demand to reproduce unavailable payload bytes. It proves that Receipts, causation, replay/resync, provenance, and explicit availability gaps remain truthful while Redaction, Tombstone, compaction, archive, export, restore, cache, projection, and Provider continuity cannot make unavailable payload visible, eligible, or newly authoritative.
_Avoid_: Byte-identical redacted replay, tombstone-only assertion, cache resurrection, silent availability gap

**Physical Deletion Completion**:
The lifecycle fact recorded only after every StoryOS-controlled online, archive, and Recovery Copy retention window authorized for an erased payload has expired or been verifiably cleaned. It does not claim deletion from an already delivered Project Export or external destination and never changes the earlier logical Redaction or Tombstone effect.
_Avoid_: Immediate disk wipe claim, Provider erasure, logical redaction alone

**Project Deletion Request**:
The explicit author-owned command that begins deletion of one exact Project Scope, atomically preventing new Runs, outbox dispatch, Context Assembly, Export, Restore, and outbound disclosure while existing work enters controlled cancellation or recovery settlement. It is never inferred from a Retention Profile, archive threshold, or cache cleanup.
_Avoid_: Automatic project expiry, table drop, bulk cache eviction, implicit account deletion

**Project Deletion Settlement**:
The immutable terminal lifecycle decision reached after a Project Deletion Request records every known in-flight operation as settled or OutcomeUnknown, fences future work, and makes the Project Scope logically unreadable, unexecutable, unexportable, and unrestorable. It starts physical cleanup under the Retention Profile and preserves only the minimum deletion, availability, and external-effect evidence until Physical Deletion Completion.
_Avoid_: Immediate disk wipe, cancelled request, restored Project, forgotten external effect
