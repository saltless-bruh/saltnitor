# Saltnitor vNext — Spec Pack & Audit Report

> **Pack:** `saltnitor-vnext` · **Revision:** r2 (post-audit, verified) · **Date:** 2026-09-27
> **Inputs:** [BP] `SALTNITOR_VNEXT_BLUEPRINT.md` · [TP] *Technical Proposal — Saltnitor Qwen MoE Runtime, Tuning Laboratory, and Remote Agent Server* · repository `saltless-bruh/saltnitor` @ `c89f278`

## 1. Summary

**What you get**

| File | What it is |
|---|---|
| `requirements.md` | The contract. 274 active requirements (258 MUST, 15 SHOULD, 1 MAY) with **439 numbered EARS acceptance criteria**, plus invariants, decisions, open questions, baseline defects, and normative appendices. |
| `tasks.md` | The work queue agents run from. **150 tasks in 12 phases (P0–P11)**. Each task cites the requirement IDs (or single ACs) it must satisfy and ends with an exact **Done when** check. Each phase ends in a **gate (G0–G11)** with pass conditions. |
| `tools/spec_lint.py` | Stdlib-only checker that keeps the two files consistent and proves test-to-requirement traceability at each gate. |
| `tools/test_spec_lint.py` | 16 self-tests: the pack passes, and every class of seeded defect fails. |

**Audit verdict.** The first draft (r1) covered the blueprint well but was not something an agent could check itself against:

- 239 of 254 MUST requirements had no acceptance criteria.
- Nothing was written in EARS form.
- 27 tasks had no completion check.
- 8 requirements had no task.
- The `[P]` parallel markers put conflicting edits on the same file.

The 2026 research pointed to a second gap: nothing protected the tests from the agent being graded by them.

r2 fixes all of this. The final verification pass then found and fixed eleven more problems (§5). The main one: as first written, some gates could never have passed without faking tests.

**Mechanical status of the delivered pack**

- `spec_lint.py`: OK.
- Self-test: 16/16 pass.
- Every MUST acceptance criterion is covered by a task.
- No task depends on a later phase, and there are no dependency cycles.
- No `[P]` tasks share files or depend on each other.
- No gate cites a requirement that is first due in a later phase.
- Every requirement traces back to a source.

## 2. How the work was done

1. **Read everything.** Both source documents, and every file in the repository at `c89f278`. The code audit produced 32 baseline defects (`BD-01…32`), each tied to a file and line — for example, the hardcoded bearer token, `killall -9`, the buffered "streaming" proxy, and tuner keys that are never written.
2. **Drafted r1** from BP + TP. Where the two conflict, BP's migration order wins (DEC-01).
3. **Researched 2026 practice** in four areas:
   - spec-driven development tooling;
   - long-running agent harnesses;
   - agent reward hacking;
   - context limits.

   I also checked the upstream facts that the spec depends on: the llama.cpp router mode, the Codacus `perf` fork, and the client configuration formats.
4. **Audited r1** against a checklist built from that research (§4), then rewrote it as r2.
5. **Made the checks mechanical.** Rules that only a careful reviewer would notice became `spec_lint.py` errors.
6. **Verified r2** three ways: the lint, extra deep checks, and a targeted read-through. That surfaced the §5 findings, which are now fixed. The tool self-test guards against regressions.

## 3. What 2026 practice says, and where it landed

| Finding | Source | Applied in r2 |
|---|---|---|
| Write acceptance criteria as EARS sentences with requirement IDs, and link tasks back to requirements. State universal rules ("for any…") so they can become property tests. | Kiro specs docs; Mavin's EARS guide | Every AC is an EARS-pattern SHALL/SHOULD sentence (§1.1). 21 ACs are tagged `[PROP]` for `proptest`. Tasks cite `REQ-…` IDs, or single ACs. |
| State preserved behavior explicitly with "SHALL CONTINUE TO". | Kiro bugfix specs (Feb 2026) | REQ-MIG-007/AC1–AC7 pin the behavior that migration must keep: hot-swap by model id, `/v1/ensure`, the TUI features, kill-switch, crash-dump, and v1 configs. |
| Mark tasks `[P]` only when they touch different files and have no dependency. Write tests first. Put a checkpoint after every phase. Check consistency across artifacts before implementing. | GitHub Spec Kit: tasks template, `spec-driven.md`, `/speckit.analyze` | `[P]` rule is lint-enforced. 20 tasks are marked test-first. Every phase ends in a gate. `spec_lint.py` is our analyze step. |
| Treat checklists as "unit tests for English", with at least 80% of items traceable. | Spec Kit checklist command | Traceability is 100% and machine-checked: every requirement has tasks, every MUST AC has a covering task, and every gate row cites IDs. |
| Keep state outside the agent's context: a feature list, a progress file, one feature at a time, end-to-end verification. "It is unacceptable to remove or edit tests." | Anthropic, *Effective harnesses for long-running agents*; Osmani, *Long-running agents* | Session protocol, `PROGRESS.md`, one task at a time, the Definition of Done, and the Never-list in the boundaries. |
| Give the agent a check it can run. Performance degrades as context fills. | Claude Code best practices | Every task has an exact **Done when**. The standard-verification block runs before and after each task. Agents load one phase at a time. |
| Keep `AGENTS.md` short (~100 lines) as a map. Enforce invariants mechanically, with error messages that tell the agent how to fix the problem. | OpenAI, *Harness engineering* (Feb 2026); AGENTS.md field guide (May 2026: 150–200 lines, commands first, a Never-list) | REQ-DOC-005 caps `AGENTS.md` at 150 lines (skeleton in `tasks.md` Appendix B). The invariant scan must print remediation (REQ-CI-008/AC2). Lint messages say what to do. |
| Plans are living documents, with a decision log and validation written as observable behavior. | OpenAI ExecPlans (`PLANS.md`) | Decisions table (§4); change protocol (REQ-DOC-007) that amends the spec in place; changelog (Appendix I); Done-when written as commands and outputs. |
| Cover six areas (commands, tests, structure, style, git, boundaries). Use three-tier boundaries. Instruction-following degrades as instructions pile up. Keep generation separate from evaluation. | Osmani, *How to write a good spec for AI agents* | Always / Ask first / Never table. Independent verifier at each gate (REQ-TST-013). Small, per-task context. |
| Models get less reliable as input grows, even on simple tasks. At 500 simultaneous instructions the best model follows only 68%, with a bias toward earlier ones. | Chroma, *Context Rot* (Jul 2025); IFScale (arXiv 2507.11538) | Progressive disclosure: rules first and short. One phase is 0.8–3.3K tokens. A task plus its cited requirements has a median of ~350 tokens. Nothing needs the whole file. |
| Coding agents exploit tests. GPT-5 did so 76% of the time on one ImpossibleBench variant. Hiding tests brings this near zero, and an "abort" option cut one rate from 54% to 9%. Held-out compositional tests expose "passes visible tests, fails real use", and the gap grows with code size. The Sep 2026 compilation also advises: keep tests, grader and CI out of the agent's write scope; a changed assertion is a review item; never ask the agent whether it cheated. | ImpossibleBench; SpecBench (May 2026); Digital Applied compilation (Sep 2026) | Protected acceptance suites written first by an independent author, behind CODEOWNERS and branch protection (REQ-TST-012). A compositional scenario per gate (TST-014). Independent verification plus runtime falsification (TST-013). Anti-gaming rubric (TST-015). `BLOCKED:` as a legitimate outcome (rule 4). Never-list. |
| Spec and code should land together, or the commit doesn't land. | *The Spec Growth Engine* (Jun 2026) | REQ-DOC-007/AC3 (same-PR spec, test and doc updates). `spec_lint.py` runs in CI. |
| Critiques: SDD specs get verbose and hard to review; agents don't follow every instruction; workflows are elaborate. Thoughtworks rates SDD "Assess". | Böckeler (Oct 2025); Thoughtworks Radar; DEV 2026 overview | Taken seriously — see the trade-offs in §7. Mitigations: the large files are never read whole, generated fields remove hand-maintained duplication, and the lint replaces reviewer vigilance. |

**Domain corrections that came from research** (they change behavior, not just form):

- **Codacus async CPU/GPU split is on by default.**
  - A true baseline must pass `--no-sched-async-cpu` (REQ-PROF-006/AC2).
  - The best cache slot count falls from 124 to 112 to 88 as MTP and async are stacked, so capacity is re-validated after every stage that shifts VRAM (REQ-MOE-012).
  - The tuner keeps ~900 MB of VRAM free, per the fork's README (`tuner.min_free_vram_mb`).
  - The trace invocation and merge-by-concatenation follow the README (REQ-MOE-003).
- **MoE-cache capacity comes in two dialects.** It is slots per layer in Codacus (`--moe-cache-slots`) and MiB in the upstream RFC #24528 (`--moe-cache`). DEC-17 makes the knob abstract, so the tuners never hardcode either.
- **llama.cpp router mode:**
  - Status comes back as objects (`status.value`), which the current code probably misreads (BD-32, suspected).
  - `--models-max` is not race-safe (#20137, closed as not planned), so Saltnitor serializes every load and runs with `--models-max 1` (DEC-03, REQ-RT-022).
  - The phantom `default` entry (#22364) is filtered out.
  - Sampler settings in presets are ignored (#23460), so they are rejected in router mode.
- **Hermetic runtime environment.** The runtime also reads settings from `LLAMA_ARG_*` environment variables, so the runtime env is built from an allowlist (REQ-RT-021).
- **Client configs:**
  - DeepSeek Harness: `$DSH_HOME/settings.yaml` with `api: openai-completions`, `baseURL`, `apiKeyEnv`. This resolves OQ-01.
  - OpenCode: `@ai-sdk/openai-compatible` with `options.baseURL` and `{env:…}` keys.

## 4. Audit of r1 and what changed

| Criterion (from §3) | r1 | r2 |
|---|---|---|
| MUST requirements with acceptance criteria | 15 / 254 (21 AC mentions in total) | **258 / 258**; 439 ACs, 416 of them in MUST requirements |
| EARS form | none | WHEN 80 · WHILE 16 · IF…THEN 75 · WHERE 22 · SHALL CONTINUE TO 9 |
| Property-based criteria | none | 21 `[PROP]` ACs |
| Requirements with no task | 6 MUST, 2 SHOULD | 0 (every MUST AC covered; lint error if not) |
| Tasks without a completion check | 27 / 134 | 0 / 150 |
| Task dependencies | 34 tasks with none, unchecked | Every listed dependency is lint-checked. The 26 tasks with none are phase-entry or independent tasks, gated by their phase's **Entry** line. |
| Parallel `[P]` safety | 28 `[P]` tasks, unchecked. T2.2–T2.6 were all marked `[P]` yet all edit `main.rs`. | 21 `[P]` tasks. Each declares **Files**, and files are disjoint (lint-enforced). |
| Gate strength | Pass conditions only | Each gate has:<br>• a CI row;<br>• an unmodified acceptance suite written before the phase by an independent author;<br>• a `--tests` traceability row;<br>• hardware/human demo rows;<br>• a **V** row (independent verification and runtime falsification of listed claims);<br>• an evidence file;<br>• an operator-only verdict. |
| Test protection | None | CODEOWNERS and branch protection on specs, acceptance tests, fixtures, snapshots, and `gate.sh`. Never-list. Anti-gaming rubric. |
| Agent operating model | Implicit | "Start here" in `tasks.md`: session protocol, standard verification, boundaries, Definition of Done, task format, gate procedure, fix-task protocol. Plus templates and the `AGENTS.md` skeleton. |
| Consistency tooling | A one-off script | `spec_lint.py` with `--sync`, `--tests`, `--list-phase`; wired into CI in T1.3; self-tested |

**New requirement IDs in r2:**

| ID | Topic |
|---|---|
| MIG-007 | Preserved behavior |
| RT-021 | Hermetic runtime env |
| RT-022 | Router hardening |
| MOE-012 | Re-tune after VRAM-shifting stages |
| TST-012 | Protected acceptance suite |
| TST-013 | Independent gate verification |
| TST-014 | Compositional scenarios |
| TST-015 | Anti-gaming rubric |
| TST-016 | Test ↔ requirement traceability |
| DOC-006 | Spec lint |
| DOC-007 | Spec change protocol |

Also new: DEC-17, the assumptions table, and BD-32. PRX-020 went from MAY to MUST.

**Retired**, each with an in-place stub pointing to its replacement; IDs are never reused:

| Retired | Replaced by |
|---|---|
| TST-003 | SCH-010 |
| TST-004 | RT-005 |
| TST-009 | TUI-009 / MIG-005 |
| BEN-013 | SCH-008 |
| MOE-010 | INV-11 |
| REM-004 | SEC-002 |
| REM-006 | SEC-007 |
| TEL-007 | ARCH-006 |

**Phase and version plan**

| Phase | Scope | Version | Tasks | ACs first due | Gate highlights |
|---|---|---|---|---|---|
| P0 | Baseline freeze and test harness | — | 11 | 35 | Characterization + snapshots green; `src/` unchanged; fake runtime; branch protection; lint and self-test |
| P1 | Hardening | v0.2 | 17 | 105 | Real streaming; auth on every route; strict config; PID-only process control; invariant ratchet empty |
| P2 | Module extraction | v0.2.x | 11 | 8 | `main.rs` ≤ 150 lines; no test assertion changed |
| P3 | Daemon / TUI / CLI split | v0.3 | 12 | 56 | TUI exits while inference continues; headless daemon; CLI over SSH |
| P4 | Runtime backends, config v2 | v0.4 | 18 | 86 | Capabilities from probes only; switch runtimes without source edits; lossless migration |
| P5 | Telemetry v2, oracle v2 | v0.5-a | 14 | 39 | Admission reacts to external VRAM use; observed configs persist; prediction accuracy within targets |
| P6 | Residency scheduler | v0.5-b | 11 | 33 | A swap never cuts off an active stream; mutation score; proxy overhead budget |
| P7 | Benchmark lab | v0.5-c | 16 | 26 | Exact rerun from record; comparisons from stored runs only |
| P8 | MoE / Codacus tuning lab | v0.5 | 11 | 19 | Trace → capacity tuner → MTP → context sweep → ladder with re-validation |
| P9 | Remote agent serving | v0.6 | 9 | 17 | Only Saltnitor reachable on the tailnet; OpenCode streams and calls tools over Tailscale; recovery over SSH |
| P10 | Flagship validation (Qwen3.6-35B-A3B, RTX 3060) | v0.6.x | 13 | 9 | 10-stage ladder, honest report, production profile chosen on agent-benchmark evidence |
| P11 | v1.0 readiness | v1.0 | 7 | 6 | Frozen schema/API; upgrade path; reproducible release; complete CLI surface; sustained use |

`tasks.md` also records:

- 19 `[HW]` tasks, which need the reference host;
- 28 `[HUMAN]` tasks, which need your action or sign-off;
- 321 MUST acceptance criteria whose tests are machine-checked for a `Verifies:` tag by the time their gate comes up.

## 5. Verification pass — issues found in r2 and fixed

| # | Problem | Consequence if shipped | Fix |
|---|---|---|---|
| 1 | The lint treated **all** of a requirement's ACs as due at its first phase. | G0/G1 would have demanded tests for features built later — for example, crash-dump semantics (P7) and v2 auth defaults (P4). The gate could not be passed honestly, which is exactly the setup that invites fake tests. | Tasks can cite single ACs (`REQ-SEC-002/AC3`); 32 such citations were added. Each AC's due phase is computed on its own. Four compound ACs were split so each lands in one phase:<br>• MIG-007/AC5 → AC5 + AC6;<br>• SEC-003/AC3 → AC3 + AC4;<br>• CI-008/AC1 → AC1 + AC4;<br>• DOC-006/AC1 → AC1 + AC2.<br>MIG-007/AC7 makes "v1 configs keep loading until v1.0" explicit. |
| 2 | `--tests` only read Rust files. | CI and lint requirements could never be satisfied. | Tags are now read from Rust, Python, shell, and CI YAML. Markdown never counts. |
| 3 | 13 `[P]` tasks had no **Files**; 3 of them touch shared code. | Parallel agents collide. | Added Files. Dropped `[P]` from T3.11, T5.5, and T7.11. The lint now enforces the rule. |
| 4 | T0.5/T0.6 both edit `src/control_api.rs`, and T1.2/T1.16 both edit `CHANGELOG.md`/`SECURITY.md`, with no order between them. | Merge conflicts. | Dependencies added. |
| 5 | T4.8's Done-when used `runtime list --json`, which is built in T4.10. | The task can't be finished in order. | Check moved to T4.10; T4.8 is now a dependency of T4.10. |
| 6 | Six CLI commands had no owning task: `config reload`, `runtime validate`, `runtime promote`, `profile derive-baseline`, `trace`, `tune capacity`. | v1.0 would ship with stubs. | Assigned to T4.2, T4.10, T4.14, T8.4, and T8.5. G11 gains a "CLI surface complete" row. |
| 7 | Some gates were missing rows. | Inconsistent gates. | Traceability rows added to G2, G10, and G11. CI rows added to G10 and G11. G11 gains claims to falsify. G0 now runs `--tests --phase P0` and the self-test. |
| 8 | The `Verify:` field disagreed with the AC markers in 13 requirements. | Verifiers would read the wrong method. | `Verify:` is now generated from the AC markers by `--sync`. |
| 9 | NFR and DOC had no backward-traceability row, and 2 research-derived requirements had no source. | Requirements whose origin can't be traced. | Rows added to Appendix G, sources added, and the lint now checks. |
| 10 | T0.4 claimed REQ-CI-003, but CI wiring happens in T1.3. | A false claim at G0. | Removed from T0.4. |
| 11 | Storage migrations had no file convention. | Tasks that add tables collide. | One migration file per task, named with the task number (T5.4). |

## 6. Decisions and open questions for you

Your call; each one has a safe default already written into the spec.

**Open questions**

- **OQ-03 — oracle accuracy targets.** 5 / 10 / 15% per prediction tier. These are proposals; confirm them before G5.
- **OQ-04 — soak and sustained-use durations.** 24 h at the gate, 72 h before release, 14 days of daily use before v1.0. Also proposals; confirm or change them.
- **OQ-02 — Codacus flag names.** Taken from the Sep 2026 README. They are confirmed or corrected when fixtures are captured in T4.13; the fixtures win and the spec gets a change request.
- **BD-32 — suspected router-status bug.** It needs one `/models` sample from your working router (T0.3).

**Decisions that depart from a source document**

| Decision | What it says | Why |
|---|---|---|
| DEC-02 | Adds a residency-scheduler phase (P6) before the benchmark lab. BP's migration plan has no separate scheduler phase. | Lab runs need exclusive leases. |
| DEC-11 | Remote serving is built and gated in P9, but the production agent profile is promoted only after P10 validation. | This reconciles TP §42 with BP Phase 8. |
| DEC-12 | During lab runs, inference gets `MODEL_BUSY` by default; queueing is configurable. | Keeps measurements valid. |

**Standing decisions worth a second look:**

- DEC-03: one runtime process tree, router with `--models-max 1`.
- DEC-04: TOML profiles are the only source of truth; `router.ini` is generated.
- DEC-05: managed mode is the default, but a v1 config falls back to external mode.
- DEC-17: abstract MoE-cache capacity.

**What only you can do** (all tracked as `[HUMAN]`/`[HW]` tasks):

- supply the known-good `router.ini` (T0.8);
- enable branch protection (T0.10);
- approve each phase's acceptance tests before implementation starts (Tn.0);
- capture runtime fixtures on the reference host (T4.7, T4.13);
- run the hardware demos;
- sign every gate.

## 7. Trade-offs and limits

- **Size.**
  - `requirements.md` grew from ~97K to ~141K characters, and `tasks.md` from ~67K to ~95K. That is the price of 439 checkable criteria.
  - Mitigation: an agent reads the ~1.7K-token task header, one phase, and only the requirements its task cites.
  - The Böckeler and Thoughtworks critiques still apply to *human* review. Review a phase at its gate; don't read the whole pack end to end.
- **The lint checks structure, not test quality.** A tagged test can still be a weak test. That is why each gate adds three human- or runtime-level checks: an independent verifier, the anti-gaming rubric, and runtime falsification through the real binary.
- **No separate `design.md`.** You asked for requirements and tasks. Design lives in §4 decisions, the normative appendices (routes, errors, capability and parameter registries, state machines, storage), and BP/TP themselves. A `design.md` can be added later without touching IDs.
- **Upstream moves fast.** Codacus flags and llama.cpp router behavior are as documented in Sep 2026. The spec never trusts names without a captured fixture (INV-02, REQ-RT-005), so drift shows up as a failing fixture test, not as silent misbehavior.
- **Not yet exercised against code.** The pack is internally consistent and mechanically verified, but no phase has been implemented. Expect the first change requests in P0–P1 — the change protocol is there for exactly that.

## 8. How to use the pack

1. **Install it.** Copy the folder to `docs/specs/vnext/` on branch `vnext`, and put BP and TP in `docs/specs/vnext/sources/` (this is task T0.1).
2. **Check it installed cleanly.**
   - `python3 docs/specs/vnext/tools/spec_lint.py` should print `OK`.
   - `python3 -m unittest discover -s docs/specs/vnext/tools` should pass.
3. **Create `AGENTS.md`** from `tasks.md` Appendix B (≤ 150 lines).
4. **Start each agent session with** "follow *Start here* in `docs/specs/vnext/tasks.md`". The agent:
   - loads only the current phase (`P=P4; sed -n "/<!-- phase:$P -->/,/<!-- \/phase:$P -->/p" tasks.md`);
   - takes the lowest unchecked task whose dependencies are done;
   - finishes it against **Done when**, logs it, and stops.
5. **At a gate:**
   - run `scripts/gate.sh G<n>` and `spec_lint.py --tests --phase P<n>`;
   - give a fresh reviewer `spec_lint.py --list-phase P<n>`, the diff, and the evidence;
   - falsify the listed claims through the real binary;
   - then you write the verdict.
6. **To change the spec:**
   - `CHANGE_REQUESTS.md` → your approval;
   - amend the spec in place → `spec_lint.py --sync` → add an Appendix I entry.

## Sources

**Spec-driven development**
- Kiro — [Specs best practices](https://kiro.dev/docs/specs/best-practices/) · [Correctness / property-based testing](https://kiro.dev/docs/specs/correctness/) · [New spec types: bugfix and design-first](https://kiro.dev/blog/specs-bugfix-and-design-first/) (Feb 18, 2026)
- GitHub Spec Kit — [spec-driven.md](https://github.com/github/spec-kit/blob/main/spec-driven.md) · [tasks template](https://github.com/github/spec-kit/blob/main/templates/tasks-template.md) · [checklist command](https://github.com/github/spec-kit/blob/main/templates/commands/checklist.md) · [MarkTechPost overview](https://www.marktechpost.com/2026/05/08/meet-github-spec-kit-an-open-source-toolkit-for-spec-driven-development-with-ai-coding-agents/) (May 8, 2026)
- [OpenSpec](https://github.com/Fission-AI/openspec) (Fission-AI)
- Alistair Mavin — [EARS: Easy Approach to Requirements Syntax](https://alistairmavin.com/ears/)
- Birgitta Böckeler — [Understanding spec-driven development: Kiro, spec-kit, and Tessl](https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html) (Oct 15, 2025)
- Thoughtworks Technology Radar — [Spec-driven development](https://www.thoughtworks.com/en-us/radar/techniques/spec-driven-development)
- DEV Community — [Spec-driven development in 2026](https://dev.to/krlz/spec-driven-development-in-2026-what-it-is-the-tooling-and-how-teams-actually-use-it-2fk2)
- [The Spec Growth Engine](https://arxiv.org/html/2606.27045) (arXiv 2606.27045, Jun 25, 2026)

**Agent harnesses and instructions**
- Anthropic — [Effective harnesses for long-running agents](https://anthropic.com/engineering/effective-harnesses-for-long-running-agents)
- [Claude Code best practices](https://code.claude.com/docs/en/best-practices)
- OpenAI — [Harness engineering](https://openai.com/index/harness-engineering/) (Feb 11, 2026) · [ExecPlans / PLANS.md](https://github.com/openai/openai-cookbook/blob/main/articles/codex_exec_plans.md)
- Addy Osmani — [How to write a good spec for AI agents](https://addyosmani.com/blog/good-spec/) · [Long-running agents](https://addyosmani.com/blog/long-running-agents/)
- Iurii Okhmat — [The AGENTS.md field guide, 2026 edition](https://www.iuriio.com/blog/posts/2026/05/agents-md-field-guide-2026) (May 2026)

**Context and reward hacking**
- Chroma — [Context Rot](https://www.trychroma.com/research/context-rot) (Jul 14, 2025)
- Jaroslawicz et al. — [How Many Instructions Can LLMs Follow at Once? (IFScale)](https://arxiv.org/abs/2507.11538)
- [ImpossibleBench: measuring reward hacking in LLM coding](https://www.lesswrong.com/posts/qJYMbrabcQqCZ7iqm/impossiblebench-measuring-reward-hacking-in-llm-coding-1)
- [SpecBench: measuring reward hacking in long-horizon coding agents](https://arxiv.org/html/2605.21384v1) (May 20, 2026)
- Digital Applied — [How often AI coding agents cheat on tests](https://www.digitalapplied.com/blog/ai-coding-agent-reward-hacking-rates-published-data) (Sep 17, 2026)

**Runtime and clients**
- thecodacus/llama.cpp — [`perf` README](https://raw.githubusercontent.com/thecodacus/llama.cpp/perf/README.md) · [repository](https://github.com/thecodacus/llama.cpp)
- ggml-org/llama.cpp — [RFC #24528: MoE expert cache](https://github.com/ggml-org/llama.cpp/discussions/24528) (Jun 12, 2026) · [Issue #20137: `--models-max` race](https://github.com/ggml-org/llama.cpp/issues/20137) · [Issue #22364: phantom `default` model](https://github.com/ggml-org/llama.cpp/issues/22364)
- [llama.cpp router mode in 2026 (presets, API, known issues)](https://runaihome.com/blog/llama-server-router-mode-multi-model-setup-2026/)
- DeepSeek Harness — [providers guide](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/user/guide/providers.md) · OpenCode — [providers](https://opencode.ai/docs/providers/)
