# Fiction memory and research provenance semantics: source research and StoryOS implications

- Status: research complete; decision input for [Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51), not a domain decision or implementation authorization
- Accepted downstream contract: [Fiction Memory and Research Provenance Semantics](../foundation/fiction-memory-and-research-provenance-semantics.md)
- Repository baseline: `main` as inspected on 2026-07-19
- External-source check date: 2026-07-19
- Evidence policy: W3C Recommendations, official product documentation, original papers, and original scholarly monographs only

## Research question

What evidence should inform the StoryOS decision about the exact fiction-domain meanings and boundaries of canon, Candidates, episodic/semantic/procedural memory, semantic indexes, story time, epistemic scope, conflicts, supersession, source capture and citation, and derivation across the authority boundary?

The practical goal is to make the later HITL/domain-modeling discussion concrete enough to decide one question at a time without importing an unrelated cognitive taxonomy, knowledge-graph convention, or LLM-memory prototype as the StoryOS product model.

## Scope and non-goals

This report covers:

- first-party and primary evidence for provenance, citation locators, multiple or conflicting statements, temporal representation, narrative time, point of view, cognitive memory categories, LLM runtime memory, and rebuildable text indexes;
- the already accepted StoryOS Artifact, provenance, Proposal, Acceptance, and Authoritative-State boundaries;
- evidence-backed implications and precise questions that remain for the author.

This report does **not**:

- make the final fiction-domain decisions;
- define Rust structs, JSON schemas, database tables, embedding layouts, ranking formulas, or UI screens;
- choose a graph database, ontology, vector store, or retrieval algorithm;
- treat psychology or narratology as a software schema;
- authorize automatic extraction, automatic canonization, or any direct `promotion` path;
- decide physical persistence, migration, backup, indexing, context selection, or outbound disclosure rules owned by later tickets.

## StoryOS constraints at the inspected baseline

The repository is the governing source for these constraints.

1. [Authoritative State](../../CONTEXT.md) is the author-approved current truth of the project and already includes prose, established fictional-world truth, characters, relationships, timeline, and manuscript structure. Authority is binary and arises only through an explicit author-authorized domain action.
2. [ADR 0001](../adr/0001-separate-authoritative-state-artifacts-and-operational-records.md) separates Authoritative State, Artifacts, and Operational Records. An Artifact never becomes authoritative in place.
3. The accepted [Artifact and Authoritative-State Domain Model](../foundation/artifact-domain-model.md) already defines immutable Artifact Revisions, exact revision references, typed Provenance Edges, source snapshots, Claims, Candidates, Supersession, Derivation, Proposals, Validation Receipts, and Acceptance.
4. At the inspected baseline, a `Candidate` was one independently reviewable semantic fact or object with no authoritative change command, while the existing `MemoryCandidate` subtype had no settled fiction-domain meaning.
5. An `ExternalSourceSnapshot` or `ImportedSourceSnapshot` is an immutable Research Artifact Revision. A `ResearchSynthesis` binds stable Claims to exact Source Snapshot Revisions through `supported_by` edges. A Claim stays non-authoritative regardless of confidence or repetition.
6. `available_as_context` and `supported_by` are intentionally different provenance roles. Visibility to a model is not evidence that the model used the item to support a conclusion.
7. `Derivation` creates a new Artifact from exact source Artifact Revisions and never changes the source kind in place. `Supersession` preserves both Artifacts and states that one replaces another only for a stated purpose.
8. Only an eligible StoryOS Core `Proposal` can cross the authority boundary. `Acceptance` applies selected pending Proposal Operations and creates new Authoritative Revisions and an Authoritative Commit. The accepted contract explicitly rejects “promotion,” status flips, and overwrites.
9. The [Manuscript Revision and Proposal State Machine](../foundation/manuscript-revision-proposal-state-machine.md) requires immutable linear revisions, exact expected heads, explicit conflict, no silent rebase, and a Core Transition for authority changes. Audit time is evidence, not story chronology or causal order.
10. Local project data is authoritative. External model and service disclosure must be minimized, explicitly granted, attributable, and inspectable. Model-visible context is incrementally assembled, structured, attributable, and hard-bounded.
11. There is one general Agent Loop. Durable Run records, transcript Messages, Skills, model context, caches, and process memory are not a hidden second creative state.

These constraints mean that this ticket may refine fiction-domain objects and relations, but cannot reopen the accepted authority or Proposal boundary.

## Evidence labels and evidence boundary

- **External source fact**: directly stated by a standard, official document, original paper, or original monograph.
- **StoryOS-local fact**: already accepted in this repository.
- **Evidence-backed implication**: a reasoned consequence of source facts plus StoryOS constraints; not a final decision.
- **Recommendation**: a proposed direction for the HITL discussion; not an accepted contract.

Several evidence limits matter:

- W3C PROV describes provenance, not truth, creative authority, or conflict resolution.
- Wikibase demonstrates a workable statement/reference/qualifier/rank separation, but its community consensus and query behavior are not StoryOS author authority.
- OWL-Time supplies general temporal vocabulary, not narrative discourse, character knowledge, or a required storage ontology.
- Tulving and Cohen/Squire classify human memory systems and experimental dissociations. They do not specify software persistence types.
- Genette analyzes narrative discourse. The book is cited to the original Cornell University Press edition and page/section locations; full text was not assumed available through the publisher page.
- Generative Agents and MemGPT are research prototypes evaluated for particular tasks. They are evidence about LLM context-management patterns, not product specifications or authority models.

## External source facts

### 1. Provenance records production and influence; it does not certify truth

The W3C PROV Data Model defines provenance as records about entities, activities, and agents involved in producing, influencing, or delivering a thing. It says such records can support assessments of quality, reliability, and trustworthiness; it does not say provenance itself establishes those judgments. PROV is explicitly domain-agnostic and provides extension points for application-specific semantics. See [PROV-DM, W3C Recommendation 30 April 2013, Abstract and sections 1 and 2](https://www.w3.org/TR/2013/REC-prov-dm-20130430/).

PROV distinguishes generation, usage, derivation, attribution, association, and delegation. A derivation means one entity was transformed into, updated into, or used to construct another, and a fully qualified derivation can name the activity, usage, and generation involved. PROV warns that mere co-occurrence as an activity input and output is insufficient to establish derivation; influence is also required. See [PROV-DM sections 2.1.2 and 5.2.1](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Derivation) and [PROV-O qualified derivation, W3C Recommendation 30 April 2013](https://www.w3.org/TR/2013/REC-prov-o-20130430/#qualifiedDerivation).

PROV models `Revision`, `Quotation`, and `Primary Source` as special kinds of derivation. A PROV primary source is relational and topic-dependent; the specification states that identifying one is partly interpretive and should follow domain conventions. It is not a universal trust badge on an entity. See [PROV-DM sections 5.2.2–5.2.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-primary-source).

PROV invalidation means an entity ceases to be available for use after an invalidating activity. That is different from StoryOS supersession, historical preservation, archival exclusion, and payload tombstoning, which already have separate local semantics. See [PROV-DM section 5.1.8](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Invalidation).

### 2. Citation needs both an exact source state and a locator

The W3C Web Annotation Data Model separates the body of an annotation from its target and uses a `SpecificResource` when the intended target is a segment, representation, or state of another resource. A Selector identifies a segment; a State identifies the intended representation. See [Web Annotation Data Model, W3C Recommendation 23 February 2017, sections 3–4](https://www.w3.org/TR/2017/REC-annotation-model-20170223/).

The model provides text-quote, text-position, range, fragment, XPath, CSS, data-position, and other selectors. It explicitly warns that position selectors are brittle when the source changes and recommends also identifying the intended State. Text-quote selection can carry exact text with prefix/suffix context, but copying source text may create rights risks. See [Web Annotation sections 4.2.4–4.2.9](https://www.w3.org/TR/2017/REC-annotation-model-20170223/#selectors).

A Time State records when a source representation is appropriate and may link a cached copy; it exists because a live Web resource can change. State processing precedes selector processing. See [Web Annotation section 4.3, especially 4.3.1](https://www.w3.org/TR/2017/REC-annotation-model-20170223/#states).

These are source and locator mechanics. They do not say an annotation body is correct or that cited evidence becomes project truth.

### 3. Multiple statements, qualifiers, references, and ranking are distinct concerns

Wikibase models a Statement as a Claim plus references and rank. The Claim contains a property, value, and optional qualifiers; qualifiers change or refine what the statement means, including temporal validity and determination method. Multiple statements may coexist for one property, including divergent viewpoints. See the official [Wikibase Data Model Primer, Statements and Qualifiers](https://www.mediawiki.org/wiki/Wikibase/DataModel/Primer#Statements) and the technical [Wikibase Data Model, section 7](https://www.mediawiki.org/wiki/Wikibase/DataModel#Statements).

The Wikibase primer states both that references support a Claim and that a referenced Claim is not therefore true; trust remains a reader judgment. It also distinguishes a known `no value`, a known but unspecified value, and absence of a statement. See [Wikibase Data Model Primer, lines of discussion under Statements](https://www.mediawiki.org/wiki/Wikibase/DataModel/Primer#Statements).

Wikibase ranks (`preferred`, `normal`, `deprecated`) primarily influence query/default-display selection among Statements. More than one Statement may have the same rank. Official Wikidata guidance says references indicate where a value comes from, while rank communicates a consensus choice for query purposes; a normal rank makes no accuracy or currency judgment. See [Wikidata Help:Ranking, sections “Rank vs. references” and “Usage”](https://www.wikidata.org/wiki/Help:Ranking).

RDF named graphs do not solve epistemic scoping by themselves. RDF 1.1 defines a named graph as a graph syntactically paired with an IRI or blank node, but imposes no formal restriction on what the graph name denotes or on its relationship to the graph. Application semantics must provide that meaning. See [RDF 1.1 Concepts, W3C Recommendation 25 February 2014, section 4](https://www.w3.org/TR/2014/REC-rdf11-concepts-20140225/#section-dataset).

### 4. Story chronology, narrative discourse, and viewpoint are separate dimensions

OWL-Time represents temporal entities as instants or intervals and supplies qualitative interval relations such as before, after, meets, overlaps, starts, during, finishes, and equals. It supports Gregorian time, numeric coordinates, named/ordinal periods, and explicit temporal reference systems. See [OWL-Time, W3C Recommendation 19 October 2017, sections 3.1–3.3](https://www.w3.org/TR/2017/REC-owl-time-20171019/#time:TemporalEntity).

OWL-Time also documents its limits: temporal reference-system taxonomies are out of scope; temporal vagueness is not explicitly handled; and valid time is not a built-in resolved concept. See [OWL-Time section 4.1.18 and Appendix B](https://www.w3.org/TR/2017/REC-owl-time-20171019/#time:TRS). It therefore supplies useful primitives but does not settle a fiction timeline contract.

Gérard Genette distinguishes the story (narrative content), the narrative (the telling text/discourse), and the narrating act/situation. His temporal analysis then treats order, duration, and frequency as relations between story and narrative; mood includes focalization, while voice concerns the narrating situation. See Gérard Genette, *Narrative Discourse: An Essay in Method*, trans. Jane E. Lewin, Cornell University Press, 1980, pp. 25–32, chapters “Order” (from p. 33), “Duration” (from p. 86), “Frequency” (from p. 113), “Mood” (from p. 161), and “Voice” (from p. 212); publication and contents are verifiable through [Cornell University Press distribution](https://utpdistribution.com/9780801410994/narrative-discourse/) and [Google Books](https://books.google.com/books?id=yEPuQg7SOxIC).

Genette's focalization analysis distinguishes the regulation/source of narrative information from the identity of the narrator. The important software implication is limited: story chronology, order of telling, narrator, and viewpoint cannot safely share one undifferentiated `time` or `scope` field. The monograph does not prescribe a database schema.

Janyce Wiebe's original computational study of third-person fiction distinguishes objectively narrated fictional-world information from passages that present a character's thoughts, perceptions, emotions, beliefs, and intentions. It argues that a system must identify the current psychological point of view to distinguish character beliefs from story facts and attribute attitudes to their source; many such passages are not explicitly marked, so classification depends on context. See [Wiebe, “Tracking Point of View in Narrative,” *Computational Linguistics* 20(2), 1994, pp. 233–287, especially pp. 233–239](https://aclanthology.org/J94-2004/) and the [original paper PDF](https://aclanthology.org/J94-2004.pdf).

Wiebe's algorithm is evidence that epistemic attribution is a real, context-sensitive problem. Its output is not author authorization, and its “objective” category in a constrained text-analysis experiment must not be equated automatically with StoryOS Authoritative State.

### 5. Episodic, semantic, and procedural are human-memory distinctions

Endel Tulving's original 1972 chapter proposed episodic and semantic memory as parallel, partially overlapping information-processing systems. Episodic memory concerns personally experienced, temporally located events and their temporal-spatial relations; semantic memory concerns organized knowledge of symbols, meanings, relations, rules, and concepts. See Endel Tulving, “Episodic and Semantic Memory,” in *Organization of Memory*, ed. Endel Tulving and Wayne Donaldson, Academic Press, 1972, pp. 381–403; an accessible scan of the original chapter is available at [Episodic and Semantic Memory](https://alicekim.ca/EMSM72.pdf).

Tulving later related procedural, semantic, and episodic systems to different kinds of consciousness and emphasized the self-in-subjective-time aspect of episodic recollection. See Endel Tulving, “Memory and Consciousness,” *Canadian Psychology* 26(1), 1985, pp. 1–12, [DOI 10.1037/h0080017](https://doi.org/10.1037/h0080017). This later refinement is a warning against reducing “episodic” to any timestamped event row.

Neal Cohen and Larry Squire found that amnesic participants acquired and retained a mirror-reading skill despite impaired declarative memory. They used this evidence to distinguish rule/procedure-based “knowing how” from data-based/declarative “knowing that.” See [Cohen and Squire, “Preserved learning and retention of pattern-analyzing skill in amnesia,” *Science* 210(4466), 10 October 1980, pp. 207–210, DOI 10.1126/science.7414331](https://pubmed.ncbi.nlm.nih.gov/7414331/).

These sources describe human cognitive organization and neuropsychological dissociation. They do not imply that a novel-writing application should create three top-level persistent object families with the same names. In particular:

- a fictional character's remembered episode is an authored proposition about that character, not the Host's memory of a Run;
- project knowledge about the fictional world is not automatically human semantic memory;
- a Skill, author instruction, style guide, Agent policy, or executable routine is not automatically human procedural memory;
- a timestamped transcript or Run Event is not automatically episodic memory.

### 6. LLM “memory” is runtime context management, not project truth

The Generative Agents paper describes an experimental architecture that stores a natural-language record of simulated-agent experiences, derives higher-level reflections, and retrieves records by recency, importance, and relevance to support planning. See [Park et al., “Generative Agents: Interactive Simulacra of Human Behavior,” arXiv:2304.03442v2, 6 August 2023, Abstract](https://arxiv.org/abs/2304.03442) and paper sections 4–5.

MemGPT describes “virtual context management”: moving information among memory tiers so an LLM with a limited context window can operate over larger conversation or document history. See [Packer et al., “MemGPT: Towards LLMs as Operating Systems,” arXiv:2310.08560v2, 12 February 2024, Abstract and section 2](https://arxiv.org/abs/2310.08560).

Both papers use “memory” operationally for agent behavior and context-window management. Neither supplies an author-owned creative-truth boundary, Proposal/Acceptance gate, exact source-snapshot citation contract, local-first disclosure policy, or durable recovery contract suitable for StoryOS. Retrieval scores and model-written reflections are heuristic runtime inputs, not authority.

### 7. Full-text and semantic indexes are derived access structures

SQLite FTS5 can maintain an external-content index over a separate content table. The official documentation says the application is responsible for keeping them consistent, documents unintuitive results when they diverge, and provides a `rebuild` command that discards and recreates the full-text index from the content table. See [SQLite FTS5 documentation, sections 4.4.3, 4.4.4, and 6.12](https://www.sqlite.org/fts5.html#external_content_tables).

This is direct evidence for one narrow boundary: an FTS index can be treated as a replaceable projection when authoritative content and exact source records exist elsewhere. It does not prove that every vector or graph index is safely rebuildable; rebuildability requires pinned inputs, transformation/tokenizer/embedding versions, and no unique user-owned payload living only in the index.

## StoryOS-local facts

### Authority and fiction truth

- “Canon” is already rejected as a synonym for the whole `Authoritative State` because it is too narrow. The ticket still needs to decide what fiction-domain statements authors inspect and change inside that broader authoritative space.
- Direct author manipulation and Proposal plus Acceptance are already exhaustive authority-changing routes. Evidence, extraction, repeated retrieval, model confidence, and source rank add no third route.

### Artifact and provenance boundary

- Exact Artifact Revisions, Authoritative Revisions, snapshots, digests, creators, and typed Provenance Edges already exist conceptually.
- `supported_by` is an evidentiary relation; `derived_from` is a transformation relation; `available_as_context` is only context availability; `responds_to` is a conversational/goal relation.
- Research Artifacts, Claims, Findings, Candidates, Drafts, and Analysis Reports remain non-authoritative.
- The existing Source Snapshot contract is stronger than a URL-only citation: it retains captured content, retrieval/import provenance, digest, and available locators.
- Tombstoning preserves minimum identities and exposes a `PurgedSourceRef`; loss of evidence becomes visible and never silently deletes downstream Authoritative State.

### Identity, conflict, and supersession

- One Artifact or authoritative object has an immutable, single-parent, linear revision history. Alternatives and merges receive new identities with provenance.
- A stale expected head conflicts; StoryOS never silently overwrites, branches the same identity, or automatically rebases.
- Artifact Supersession is purpose-qualified and history-preserving. It does not mean that the earlier Artifact never existed or was globally false.
- Proposal Conflict is already exact-target drift. The ticket still needs a different fiction-domain answer for apparently contradictory statements that may instead differ by story time, epistemic holder, source, or genuine incompatibility.

### Context and runtime boundary

- Step Snapshots and Context Manifests preserve exact bounded inputs to an Agent decision; they do not become creative state.
- Run Events, transcript Messages, Subrun Mailboxes, Agent decisions, and retrieval projections are inspectable execution/context records, not hidden persistent Agent memory.
- Any later automatic extraction or retrieval can create Artifacts or Proposals only within the existing Tool/Run capability and provenance boundaries.

## Evidence-backed implications, not decisions

### Implication 1: “Memory” should not be the aggregate above all fiction knowledge

The external sources use “memory” for at least three incompatible things: human cognitive systems, a fictional character's subjective experience, and LLM context management. StoryOS already uses `Authoritative State`, Artifacts, Operational Records, Context Manifests, and indexes for distinct durable roles.

**Recommendation:** reserve any fiction-memory term for a narrowly defined story-domain concept, if the HITL discussion finds one necessary. Do not rename all canon, research, transcript history, or retrieval state to memory.

### Implication 2: cognitive labels are prompts for questions, not ready-made product types

Tulving's episodic/semantic distinction and Cohen/Squire's declarative/procedural dissociation show that event recollection, generalized knowledge, and learned performance can differ. But StoryOS must ask whose episode, whose knowledge, and whose procedure is being represented.

**Recommendation:** test each proposed use of `episodic`, `semantic`, or `procedural` against a concrete fiction example and a counterexample before admitting it to the glossary. If a plain domain term such as character recollection, fictional-world assertion, character skill, author instruction, or retrieval projection is clearer, prefer it.

### Implication 3: epistemic holder and authority actor are orthogonal

Wiebe shows that a sentence may present a character's false belief while remaining part of the authored fiction. The author can authoritatively establish that “character A believes X” without authoritatively establishing X. The author who authorizes a revision is also not the same role as the narrator or character whose viewpoint the content expresses.

**Recommendation:** the later decision should preserve at least the conceptual difference among:

- who authorized the project record;
- what proposition or event the record concerns;
- whose knowledge, belief, perception, intention, recollection, or narration it represents;
- the story-time interval or point at which that attitude applies.

This is a semantic requirement, not a proposed field list.

### Implication 4: story time cannot be audit time or narrative order

OWL-Time supports instants, intervals, qualitative ordering, and non-Gregorian/ordinal reference systems. Genette separates story chronology from order, duration, and frequency of telling. StoryOS already defines Host audit time as forensic evidence only.

**Recommendation:** require the fiction-domain decision to state how it distinguishes story time, narrative/discourse position, and audit/revision time. Do not require every project to have absolute dates; qualitative or ordinal relations may be first-class requirements.

### Implication 5: contradiction requires classification before conflict handling

Wikibase demonstrates that differing statements can coexist because qualifiers, sources, times, and viewpoints differ. In fiction, two textual assertions may be:

- true at different story times;
- beliefs of different characters;
- narrator claims with different reliability;
- a Candidate fiction assertion versus established fictional-world state;
- competing non-authoritative Candidates;
- an actual invariant violation within Authoritative State.

**Recommendation:** define domain conflict only after these scopes are resolved. Never use a global last-write-wins value or a retrieval rank as conflict resolution.

### Implication 6: source support must remain claim-specific and revision-specific

PROV distinguishes derivation from mere use, and Web Annotation separates source state from segment locator. StoryOS already distinguishes `supported_by` from `available_as_context` and pins exact revisions.

**Recommendation:** retain exact Source Snapshot Revision plus a versioned locator and connect it to an addressable Claim or Candidate. A bibliography entry or live URL alone is insufficient evidence. A context manifest may prove visibility, but not support.

### Implication 7: research evidence cannot auto-create Authoritative State

Wikibase explicitly separates a statement, its reference, and query rank; a source does not make a Claim true. PROV supports trust assessment rather than certification. StoryOS already requires an author-authorized Proposal path.

**Recommendation:** source-backed research may derive a Claim, Candidate, or Proposal. It must not directly create or mutate Authoritative State, and no source count, source type, confidence score, or model judgment should make it eligible without Core validation and author Acceptance.

### Implication 8: derivation is creation; “promotion” is the wrong transition

PROV describes derivation between distinct entities and revision as a derivation subtype. StoryOS more strictly defines Derivation among exact Artifact Revisions and Acceptance as the authority-changing command.

**Recommendation:** keep the existing route:

```text
Source Snapshot / Research Synthesis / Candidate / Draft
  -> derive a new Core Proposal with exact provenance
  -> Core validation
  -> explicit author Acceptance
  -> new Authoritative Revision and Authoritative Commit
```

There should be no `promote(candidate)` or `mark_research_canonical` operation, and no Artifact kind mutation.

### Implication 9: semantic indexes should be rebuildable projections

SQLite FTS5 demonstrates both index/content divergence and full rebuild from content. Generative Agents and MemGPT demonstrate that retrieval layers are purpose-specific runtime mechanisms.

**Recommendation:** full-text, embedding, entity-link, graph traversal, recency, importance, and other retrieval structures should contain no unique author-owned truth. Their inputs, transformation version, scope, and source revision watermarks must be sufficient to detect staleness and rebuild them. Exact storage and migration mechanics belong to the storage ticket; selection, ranking, context bounds, and disclosure belong to the context ticket.

### Implication 10: a source model and a fiction-truth model should not collapse

Research claims describe what an external source says. Fiction-domain assertions describe the author-owned project. A historical source may inspire or support a proposed setting fact, but the external world's truth and the fictional world's truth can intentionally differ.

**Recommendation:** preserve derivation/support links between the two spaces while keeping their semantics distinct. This also allows alternate-history fiction, unreliable in-world documents, and deliberate deviations from research without falsifying provenance.

## Coverage matrix

| Ticket term | Existing StoryOS contract | External evidence | Gap identified at research time | Owning ticket at research time |
| --- | --- | --- | --- | --- |
| canon | Authoritative State includes canon but rejects `Canon` as its synonym; only author action or Acceptance changes it | Wikibase separates statements, qualifiers, references, and ranks; references do not make truth | What fiction-domain assertion/event/object distinctions authors need, and how “current fictional-world truth” is expressed without becoming the whole authority model | [Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51) |
| Candidate | One independently reviewable semantic fact/object; cannot be accepted directly; `MemoryCandidate` name exists | Wikibase allows multiple qualified statements; PROV distinguishes derivation from use | What exact fiction-domain content deserves a Candidate, whether `MemoryCandidate` is clear enough, and when alternatives are separate identities | Current ticket |
| episodic memory | No settled fiction-domain definition; Run/Event/transcript objects already have other names | Tulving: personal temporally situated experience plus self/time, not merely any timestamped row | Whether StoryOS needs a character/narrator recollection concept, and how it differs from a fictional event or transcript history | Current ticket |
| semantic memory | No settled fiction-domain definition | Tulving: organized human knowledge; Wikibase: qualified statements; neither equals vector search | Whether “semantic memory” adds clarity beyond fictional-world assertion, character knowledge, or derived retrieval projection | Current ticket |
| procedural memory | Skills, policy, Tool contracts, and domain objects already have separate meanings | Cohen/Squire: learned “knowing how” dissociable from declarative information | Whether any fiction-domain concept—such as a character's learned skill—needs modeling, and whether the memory label is useful | Current ticket; Agent Skills and execution procedures remain outside it |
| semantic indexes | No index is source of truth; derived caches are generally reconstructible | SQLite FTS5 can diverge and be rebuilt from content; agent-memory papers use heuristic retrieval | Which projections are permitted and what freshness/rebuild invariants are semantic versus physical | Current ticket sets non-authority boundary; [storage](https://github.com/FrankQDWang/StoryOS/issues/56) owns persistence/rebuild; [context](https://github.com/FrankQDWang/StoryOS/issues/54) owns retrieval/ranking |
| story time | Timeline is authoritative; audit time is non-causal evidence | OWL-Time supports intervals, qualitative order, ordinal reference systems; Genette separates story time from discourse | Minimum support for point/interval/relative/uncertain/repeating time and its distinction from narrative order | Current ticket; storage and protocol tickets own encodings |
| epistemic scope | Provenance has Creator but no settled fiction knowledge/belief model | Wiebe distinguishes story facts from character beliefs and attributes attitudes; RDF named graphs add no semantics alone | Which holders and attitudes exist, how scope composes with story time, narrator reliability, and unknown/false belief | Current ticket |
| conflicts | Revision and Proposal conflicts are settled; no silent rebase/overwrite | Wikibase preserves divergent qualified statements; Wiebe shows false character belief need not contradict story fact | Distinguish temporal, epistemic, Candidate, source, and true authoritative-domain contradictions; decide author-facing resolution behavior | Current ticket; deterministic test matrix later in [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60) |
| supersession | Purpose-qualified Artifact relation; both histories remain | PROV revision/derivation preserves sources; Wikibase deprecated rank is query policy, not deletion | When fiction/research objects supersede rather than revise, conflict, invalidate, archive, or simply coexist | Current ticket |
| source capture | Exact immutable external/import snapshots with retrieval/import provenance and digest | Web Annotation State identifies source representation; PROV identifies entity/activity/agent lineage | Required source identity and capture metadata at the semantic level, including unavailable/failed captures and rights-sensitive content | Current ticket sets evidence semantics; [storage](https://github.com/FrankQDWang/StoryOS/issues/56) owns bytes/layout/retention mechanics |
| citation | Claims link to exact snapshots through `supported_by`; available context is separate | Web Annotation Selectors locate segments and warn about source drift and copied-text rights | Required locator kinds, locator versioning, claim granularity, and behavior when a locator or source payload is purged | Current ticket; protocol owns DTOs; context owns model-visible citation projection |
| promotion | Explicitly rejected; Artifact does not become authority in place | PROV derivation connects entities, not authority; Wikibase rank is not truth authorization | No new gap unless terminology elsewhere still implies promotion; current ticket should reaffirm rejection for fiction/research flows | Current ticket reaffirmation; Proposal/Acceptance contract remains authoritative |
| derivation | New Artifact from exact source Revisions; source kind/history unchanged | PROV qualified derivation can identify activity, usage, and generation | Fiction-specific derivation roles and when synthesis, Candidate, or Proposal identity must be new | Current ticket for semantics; protocol/storage own encoding |
| LLM/Agent runtime memory | Context Manifest, Step Snapshot, transcript, Run/Event, cache, and Agent records already have typed roles; no hidden persistent Agent memory | Generative Agents and MemGPT use “memory” for heuristic retrieval/context management | Which runtime projections are allowed and inspectable, without inventing a second creative state | [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54), constrained by this ticket |

## Questions prepared for the HITL decision session

These were ordered so the later grilling could ask one question at a time. Each question was a real decision; the suggested examples were tests, not proposed schemas.

1. **What is the smallest fiction-domain unit of author-approved truth?** Is it useful to speak of an addressable assertion, event, state, relationship, constraint, or some smaller set? Test with “Mara is seventeen,” “Mara believes she is seventeen,” and “a later chapter reveals that the record is false.”
2. **Which epistemic holders and attitudes are first-class?** At minimum test character knowledge, belief, uncertainty, false belief, perception, recollection, intention, and narrator assertion. Decide how narrator reliability is represented without confusing a narrated claim with project truth or derived analysis.
3. **What does “fiction memory” mean, if anything?** Choose among a character's remembered experience, an in-world record, a proposed extracted fact, or no canonical product term. Decide explicitly whether `MemoryCandidate` should survive, narrow, or be replaced by clearer domain vocabulary.
4. **What minimum story-time expressiveness is required?** Test absolute dates, invented calendars, chapter-relative order, before/after with no date, overlapping intervals, repeated/habitual events, uncertain bounds, flashback order, and timeless facts.
5. **How does time qualify truth and epistemic state?** Decide whether “X is true during interval T,” “A believes X during T,” and “the narrative reveals X at discourse position D” are distinct relations and what changes count as revision versus a new assertion.
6. **Which apparent contradictions are allowed to coexist?** Establish a decision order for different story time, different epistemic holder, unreliable narrator, competing Candidates, and irreconcilable Authoritative-State claims.
7. **When is replacement a revision, supersession, or new alternative?** Test corrected source metadata, a new research synthesis, a changed Claim, a retconned fictional fact, and two intentional alternatives. Preserve the accepted linear-identity rule.
8. **What source evidence is mandatory for a Research Claim?** Decide the minimum exact snapshot, locator, capture/import identity, retrieval time, digest, source identity, and rights/disclosure evidence, plus the behavior when capture is prohibited or only metadata can be retained.
9. **What locator contract is durable enough?** Decide which source kinds need page/section, fragment, text quote, text position, timestamp, table cell, or other locators; require them to bind an exact snapshot representation rather than a mutable live resource.
10. **What fiction-domain derivations are named?** Decide when a Research Claim derives a Candidate, when a Candidate or Draft derives a Proposal, and when an edited synthesis or alternative receives a new identity. Do not add an authority transition to this list.
11. **Which indexes are guaranteed rebuildable?** Identify the canonical inputs and transformer versions for full-text, embedding, entity, temporal, and graph projections. Decide what the system does when an index is stale, partially rebuilt, or produced by a retired model.
12. **What can enter model context?** After the domain distinctions are settled, hand exact inclusion, ranking, boundedness, provenance display, and outbound-disclosure questions to the context ticket; do not encode those heuristics as fiction truth.

## Rejected or unsafe inferences

1. **“Human memory has episodic, semantic, and procedural categories, so StoryOS needs three matching aggregate types.”** The cited research classifies human cognition, not authoring software.
2. **“Any timestamped log is episodic memory.”** Tulving's episodic concept includes personally experienced events and self-relative subjective time; a Run Event remains an Operational Record.
3. **“A knowledge graph or embedding store is semantic memory.”** “Semantic” in cognitive science, graph semantics, and vector similarity are different uses of the word.
4. **“Skills, prompts, and Tool procedures are procedural memory.”** They already have distinct StoryOS contracts; Cohen/Squire's category does not merge them.
5. **“A character's memory, the author's project memory, and the Agent's long-term memory are one thing.”** They have different subjects, authority, lifecycles, and failure modes.
6. **“Generative Agents or MemGPT supplies StoryOS's memory architecture.”** Their prototype stores and retrieval heuristics do not provide StoryOS authority, source, disclosure, retention, or recovery guarantees.
7. **“A cited or repeatedly corroborated research Claim becomes canon.”** References and provenance help assessment but do not authorize truth. StoryOS still requires Proposal, validation, Author Intent, and Acceptance.
8. **“Preferred rank or top retrieval score chooses the true value.”** Wikibase rank is query/community policy; similarity and relevance are retrieval signals. Neither is StoryOS authority.
9. **“An RDF named graph gives character or narrator epistemic semantics.”** RDF explicitly leaves graph-name meaning to applications.
10. **“A live URL is a source snapshot.”** Mutable representations and source drift require exact captured content/state, identity, digest, and locators.
11. **“A citation locator can float to the latest source revision.”** Historical support must continue to resolve the exact representation inspected at the time.
12. **“PROV `wasDerivedFrom` means StoryOS `supported_by`.”** PROV itself distinguishes derivation from mere use; StoryOS intentionally separates transformation, support, and context availability.
13. **“PROV invalidation is StoryOS supersession or deletion.”** These have different availability, retention, and history semantics.
14. **“OWL-Time should be copied wholesale as the fiction timeline schema.”** It provides useful primitives but leaves reference-system taxonomy, vagueness, and application semantics open.
15. **“Narrator discourse is fictional-world truth.”** Focalization, voice, unreliability, and character attitudes require attribution; even computational classification is not author authorization.
16. **“Derive and then promote.”** Derivation creates another non-authoritative Artifact. Only an accepted Proposal creates Authoritative Revisions.
17. **“The index can be canonical because it is persisted.”** Persisted projections can still be stale or inconsistent; unique truth must remain in the authoritative objects, exact Artifacts, and Operational Records they index.
18. **“Provenance prevents hallucination.”** Provenance makes sources and transformations inspectable; it does not guarantee that extraction, interpretation, or a cited source is correct.

## Recommended boundary for the later decision session

The current ticket should decide only durable fiction/research semantics:

- fiction-domain unit(s) of truth;
- epistemic holders/attitudes and their story-time scope;
- whether any narrow “fiction memory” term is needed;
- conflict, revision, alternative, and supersession meanings;
- claim-specific source support and fiction-specific derivation roles;
- the invariant that indexes are non-authoritative, detectably stale, and rebuildable.

It should hand off without deciding:

- physical SQLite/blob/index layout, migrations, backups, rebuild transactions, encryption, and deletion execution to [Specify the Self-Contained Project Storage and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56);
- retrieval algorithms, relevance/ranking, context budgets, author inspection of context, and outbound disclosure to [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54);
- exact DTOs, schema versions, cursors, and compatibility rules to [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58);
- deterministic validation, conflict, stale-index, recovery, and adversarial test gates to [Define Deterministic Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60).

## Source ledger

### Normative specifications and official product documentation

- W3C, [PROV-DM: The PROV Data Model](https://www.w3.org/TR/2013/REC-prov-dm-20130430/), Recommendation, 30 April 2013.
- W3C, [PROV-O: The PROV Ontology](https://www.w3.org/TR/2013/REC-prov-o-20130430/), Recommendation, 30 April 2013.
- W3C, [Web Annotation Data Model](https://www.w3.org/TR/2017/REC-annotation-model-20170223/), Recommendation, 23 February 2017.
- W3C, [Time Ontology in OWL](https://www.w3.org/TR/2017/REC-owl-time-20171019/), Recommendation, 19 October 2017.
- W3C, [RDF 1.1 Concepts and Abstract Syntax](https://www.w3.org/TR/2014/REC-rdf11-concepts-20140225/), Recommendation, 25 February 2014.
- Wikimedia, [Wikibase Data Model](https://www.mediawiki.org/wiki/Wikibase/DataModel) and [Data Model Primer](https://www.mediawiki.org/wiki/Wikibase/DataModel/Primer), living official conceptual documentation; checked 19 July 2026.
- Wikidata, [Help:Ranking](https://www.wikidata.org/wiki/Help:Ranking) and [Help:Statements](https://www.wikidata.org/wiki/Help:Statements), official project guidance; checked 19 July 2026.
- SQLite, [FTS5 Extension](https://www.sqlite.org/fts5.html), official documentation; checked 19 July 2026.

### Original scholarly sources

- Cohen, Neal J., and Larry R. Squire. [“Preserved learning and retention of pattern-analyzing skill in amnesia: dissociation of knowing how and knowing that.”](https://pubmed.ncbi.nlm.nih.gov/7414331/) *Science* 210(4466), 1980, pp. 207–210. DOI 10.1126/science.7414331.
- Genette, Gérard. [*Narrative Discourse: An Essay in Method*](https://books.google.com/books?id=yEPuQg7SOxIC). Translated by Jane E. Lewin. Cornell University Press, 1980. ISBN 9780801492594.
- Packer, Charles, et al. [“MemGPT: Towards LLMs as Operating Systems.”](https://arxiv.org/abs/2310.08560) arXiv:2310.08560v2, 2024.
- Park, Joon Sung, et al. [“Generative Agents: Interactive Simulacra of Human Behavior.”](https://arxiv.org/abs/2304.03442) arXiv:2304.03442v2, 2023.
- Tulving, Endel. [“Episodic and Semantic Memory.”](https://alicekim.ca/EMSM72.pdf) In *Organization of Memory*, edited by Endel Tulving and Wayne Donaldson, Academic Press, 1972, pp. 381–403.
- Tulving, Endel. [“Memory and Consciousness.”](https://doi.org/10.1037/h0080017) *Canadian Psychology* 26(1), 1985, pp. 1–12.
- Wiebe, Janyce M. [“Tracking Point of View in Narrative.”](https://aclanthology.org/J94-2004/) *Computational Linguistics* 20(2), 1994, pp. 233–287.

## Research conclusion

The evidence narrows the later domain discussion substantially:

- StoryOS does not need a general “memory system” above its existing Authoritative State, Artifacts, Operational Records, and Context Manifests.
- Human episodic/semantic/procedural categories are useful counterexample generators but unsafe as automatic product types.
- Fiction truth, character/narrator epistemic attitudes, research Claims, and Agent runtime context require separate semantics even when they refer to the same proposition.
- Story time, narrative order, viewpoint, and audit time are orthogonal.
- Exact source snapshots and claim-level locators improve inspectability but cannot authorize truth.
- Conflicts must be classified by time, epistemic holder, source, and authority before resolution.
- Full-text, embedding, graph, and other semantic indexes can only be projections/caches: no unique truth, explicit source revisions and transformer versions, detectable staleness, and a rebuild path.
- Research and Candidates may derive Proposals; only explicit author Acceptance may create Authoritative Revisions. There is no direct promotion path.

The bounded HITL/domain-modeling session subsequently resolved these questions in the accepted [Fiction Memory and Research Provenance Semantics](../foundation/fiction-memory-and-research-provenance-semantics.md). This report remains research evidence rather than the accepted contract.
