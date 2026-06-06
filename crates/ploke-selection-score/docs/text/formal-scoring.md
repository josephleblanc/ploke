---
title: Formal scoring mechanisms for Ploke arXiv cs.AI 2026-06-02
created: 2026-06-02
updated: 2026-06-02
type: summary
tags: [ploke, paper, research, scoring, evaluation, synthesis, year/2026]
sources:
  - formal-scoring-equation-audit.md
  - scoring-mechanisms.md
  - symbolic-theses.md
  - ../review/review-log.md
  - ../papers/
  - ../texts/
confidence: medium
contested: true
---

# Formal scoring mechanisms

Campaign: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/index|Ploke arXiv cs.AI 2026-06-02 triage]]. Companion notes: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-equation-audit|formal scoring equation audit]], [[queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/scoring-mechanisms|scoring mechanisms]], [[queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/symbolic-theses|symbolic theses]], [[queries/research/ploke-arxiv-cs-ai-2026-06-02/review/review-log|review log]].

Scope: this page formalizes paper-internal scoring, reward, verifier, benchmark, selection, admission, and evaluation mechanisms from the campaign notes. It intentionally excludes campaign routing metadata such as `interest_score`, cron priority, category-map placement, review status, and Kanban priority. Exactness labels are inherited from the source audit: `faithful-formalization`, `interpretive-compression`, `not-safely-formalizable`, or an explicit scope-limited variant.

## Reading rules and caveats

- A MathJax block below means the audit found a source-visible formula or a source-supported formal decision rule; it does not mean the whole paper is mathematically proved.
- When a paper exposes a protocol but not a scalar objective, this page represents only the supported predicate, set, threshold, or argmax logic.
- Unresolved and candidate mechanisms remain in [[#Separate unresolved and candidate mechanisms]] and are not upgraded to exact formulas.
- Symbols are defined locally in each mechanism and consolidated in [[#Consolidated symbol glossary]].

## Domain-term glossary

| Term | Definition in this page |
| --- | --- |
| admission rule | A paper-internal rule deciding whether an artifact, answer, plan, route, or research output is accepted, escalated, or rejected. |
| benchmark score | A paper-defined measurement protocol over episodes, tasks, traces, or datasets; not the campaign's triage score. |
| verifier | A human, program, handler, judge, or source predicate that checks whether an answer, artifact, trace, or claim satisfies the paper's acceptance condition. |
| reward | A scalar, binary, or component signal used inside a paper's training or routing setup; not a campaign priority score. |
| precision / recall / F1 | Standard evaluation metrics for correctness among accepted positives, coverage of true positives, and their harmonic-mean-style balance where source notes use F1. |
| KL | Kullback-Leibler divergence between distributions; used here only where the source exposes a KL loss or penalty. |
| BCE | Binary cross-entropy for a binary prediction target. |
| AIC / JSD / ELPD-LOO | VESTA model-selection metrics: Akaike information criterion, Jensen-Shannon divergence, and expected log predictive density under leave-one-out evaluation. |
| PG / SG / GG | AGENTCL plasticity gain, stability gain, and generalization gain. |
| pass@k | Benchmark success across $k$ sampled attempts; named only when a source note uses it, and not promoted here unless an anchored formula exists. |
| trace localization | Assigning success, failure, validity, or error responsibility to steps/spans inside an agent trajectory or interaction trace. |
| cost-aware routing | Route selection that trades predicted benefit against token/compute cost. |
| exact sub-mechanism | A formula visible in the paper text even when the paper's whole protocol is not safely reducible to one equation. |
| faithful-formalization | The note/text supports the formula or decision logic directly enough to render it in MathJax without adding hidden assumptions. |
| interpretive-compression | The note/text supports the mechanism, but this page compresses prose, algorithm steps, or named metrics into a compact formal shape. |
| not-safely-formalizable | The current note/text does not justify a final equation or theorem-strength claim. |
| predicate formalization | A MathJax logical condition used when the source supports pass/fail or validation structure but not a numeric formula. |
| paper-internal score | A score, reward, verifier, metric, rule, or objective defined by a paper for its own method or evaluation. |

## 1. Exact or scope-limited equation mechanisms

### 2606.00007 — Deliberative Curation: multi-agent KB governance

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00007__deliberative-curation-a-protocol-for-multi-agent-knowledge-bases|2606.00007 Deliberative Curation]]

Mechanism name: reputation, EigenTrust, voting weight, fast-track admission, and simulation metrics.

Source anchors: `texts/2606.00007.txt` lines 273-340, 384-412, and 817-936; [[queries/research/ploke-arxiv-cs-ai-2026-06-02/review/review-log|review log]] lines 113 and 160; [[queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-equation-audit|audit]] section `2606.00007`.

Exactness label: exact sub-mechanisms are source-visible; the whole protocol remains `not-safely-formalizable` as a single objective.

MathJax:

$$
r(a)=\frac{\alpha_a}{\alpha_a+\beta_a}
$$

$$
\alpha_a(t)=\alpha_a(t_0)e^{-\delta(t-\tau_{last})},\qquad
\beta_a(t)=\beta_a(t_0)e^{-\delta(t-\tau_{last})}
$$

$$
C_{ij}=\frac{\max(s_{ij},0)}{\sum_k \max(s_{ik},0)}
$$

$$
\vec t^{(k+1)}=(1-\epsilon)C^T\vec t^{(k)}+\epsilon\vec p
$$

$$
w_i=\gamma r_i+(1-\gamma)t_i
$$

$$
\operatorname{fastTrack}(c)=
\begin{cases}
\operatorname{active}(c), & \neg\exists o:\operatorname{tier}(o)\ge 1\land \operatorname{arrives}(o,c,t_{fast}) \\
\operatorname{escalate}_{Tier2}(c), & \exists o:\operatorname{tier}(o)\ge 1\land \operatorname{arrives}(o,c,t_{fast})
\end{cases}
$$

$$
\mathrm{Precision}=\frac{|\{c:c.state=active\land q(c)\ge 0.7\}|}{|\{c:c.state=active\}|}
$$

$$
\mathrm{Recall}=\frac{|\{c:c.state=active\land q(c)\ge 0.7\}|}{|\{c:q(c)\ge 0.7\}|}
$$

$$
\mathrm{FPR}=\frac{|\{a\in H\cup B:\sigma(a)\ge \sigma_2\}|}{|H\cup B|}
$$

Symbol definitions:

- $a$: agent; $c$: candidate knowledge artifact or contribution; $i,j,k$: agent or iteration indices depending on formula.
- $\alpha_a,\beta_a$: Beta reputation counts for agent $a$; $r(a)$ or $r_i$: expected reputation score.
- $t,t_0,\tau_{last}$: current time, reference time, and most recent update time; $\delta$: decay rate.
- $s_{ij}$: local trust or satisfaction score from $i$ toward $j$; $C_{ij}$: normalized nonnegative trust matrix entry.
- $\vec t^{(k)}$: global trust vector at iteration $k$; $\epsilon$: teleport or prior-mixing parameter; $\vec p$: prior trust vector.
- $w_i$: effective voting weight for agent $i$; $t_i$: EigenTrust component; $\gamma$: blend weight between Beta reputation and EigenTrust.
- $o$: objection; $t_{fast}$: fast-track objection window; $\operatorname{tier}(o)$: objection tier.
- $q(c)$: simulated artifact quality; $H,B$: honest and benign/broken agent groups in the sanction metric; $\sigma(a)$: sanction score; $\sigma_2$: Tier-2 sanction threshold.

Domain-specific definitions:

- Beta reputation: a reputation estimate built from success/failure counts.
- EigenTrust: a global trust propagation method over normalized local trust scores.
- Fast track: admission path for non-objected contributions.
- Sanction false positive: an honest or benign agent whose sanction score crosses a sanction threshold.

Explanation:

- The paper exposes exact scoring fragments for reputation, propagated trust, and vote weight.
- The fast-track rule is an admission predicate, not a scalar reward.
- Precision, recall, and FPR are simulation metrics over accepted artifacts and sanctioned agents.
- Do not collapse the whole governance protocol into one optimizer; the audit preserves unresolved Community Notes replay, sanction, dispute, and deployment caveats.

### 2606.00251 — Capability Self-Assessment

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00251__capability-self-assessment-teaching-llms-to-know-their-limits|2606.00251 Capability Self-Assessment]]

Mechanism name: CSA labels, supervised objective, binary reward, diversity filtering, and GRPO objective.

Source anchors: `texts/2606.00251.txt` lines 145-315, 342-390, and 499-577; review-log lines 115 and 162; audit section `2606.00251`.

Exactness label: `faithful-formalization` for label, reward, and objective; appendix-level CDS details remain unresolved.

MathJax:

$$
y_i=\begin{cases}
\mathrm{SELF\text{-}SOLVE}, & g\left(\{\mathbf 1[\hat a_i^{(k)}=a_i^*]\}_{k=1}^K\right)=1 \\
\mathrm{DELEGATE}, & \mathrm{otherwise}
\end{cases}
$$

$$
L_{\mathrm{SFT}}(\theta)=-\sum_{i=1}^N\log p_\theta(o_i\mid x_i)
$$

$$
R_i^{(g)}=\begin{cases}+1,&\hat y_i^{(g)}=y_i\\-1,&\hat y_i^{(g)}\ne y_i\end{cases}
$$

$$
D_{div}=\{x_i\in D:\mathrm{rollouts}(x_i)\ \mathrm{contain\ both\ SELF\text{-}SOLVE\ and\ DELEGATE}\}
$$

$$
L_{\mathrm{GRPO}}(\theta;B)=
-\mathbb E\left[\frac{1}{G}\sum_{g=1}^G
\min\left(\rho_i^{(g)}(\theta)A_i^{(g)},\operatorname{clip}(\rho_i^{(g)}(\theta),1-\epsilon,1+\epsilon)A_i^{(g)}\right)\right]
+\beta\,\mathrm{KL}[\pi_\theta\Vert\pi_{ref}]
$$

$$
\rho_i^{(g)}(\theta)=\frac{\pi_\theta(o_i^{(g)}\mid x_i)}{\pi_{\theta_{old}}(o_i^{(g)}\mid x_i)},\qquad
A_i^{(g)}=\frac{R_i^{(g)}-\operatorname{mean}_{g'}R_i^{(g')}}{\operatorname{std}_{g'}R_i^{(g')}}
$$

Symbol definitions:

- $i$: query index; $x_i$ or $q_i$: query; $a_i^*$: reference answer; $\hat a_i^{(k)}$: answer from rollout $k$; $K$: number of probes.
- $g(\cdot)$ in the label rule: aggregation function over probe correctness indicators; $y_i$: target CSA label.
- $o_i$: model output sequence; $\theta$: trainable policy parameters; $N$: supervised training set size; $p_\theta$: output likelihood.
- $\hat y_i^{(g)}$: label predicted in rollout group $g$; $R_i^{(g)}$: reward for that rollout.
- $D$: source dataset; $D_{div}$: diversity-filtered subset; $G$: group size; $B$: minibatch.
- $\rho_i^{(g)}$: policy ratio; $A_i^{(g)}$: standardized advantage; $\epsilon$: clipping parameter; $\beta$: KL penalty weight; $\pi_{ref}$: reference policy; $\theta_{old}$: old policy parameters.

Domain-specific definitions:

- CSA: capability self-assessment, deciding whether the model should solve or delegate.
- SELF-SOLVE / DELEGATE: discrete routing labels.
- DFW: diversity-filtered warm-up, using examples where rollouts produce both labels.
- GRPO: group-relative policy optimization with clipped policy ratios and KL regularization.

Explanation:

- The label rule turns repeated correctness probes into a supervised self-assessment target.
- The binary reward scores whether a rollout chose the target route, not whether it sounded confident.
- $D_{div}$ is a training-data filter to avoid zero-variance reward groups.
- CDS / Macro-F1 / Capability Ratio are evaluation terms in the note, but appendix-level CDS details remain unresolved.

### 2606.00424 — Weak Critics / O-PCD

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00424__weak-critics-make-strong-learners-on-policy-critique-distillation-for-scalable-oversight|2606.00424 Weak Critics / O-PCD]]

Mechanism name: outcome/rubric filtering and token-level KL distillation.

Source anchors: `texts/2606.00424.txt` lines 313-396; review-log lines 117 and 164; audit section `2606.00424`.

Exactness label: `faithful-formalization`; rubric-judge implementation remains partially unresolved.

MathJax:

$$
y_i\sim\pi_{\theta_e}(\cdot\mid x),\qquad f_i\sim\pi_w(\cdot\mid x,y_i)
$$

$$
r_{out}(x,\hat y_i)=\mathbf 1\{\hat y_i\text{ is correct for }x\}
$$

$$
r_{rub}(x,y_i,f_i)=\mathbf 1\{f_i\text{ is relevant and useful as revision guidance}\}
$$

$$
h_i=r_{out}(x,\hat y_i)\,r_{rub}(x,y_i,f_i),\qquad
S_e=\{(x,y_i,f_i):h_i=1\}
$$

$$
L_{OPCD}(\theta)=\frac{1}{|S_e|}\sum_{(x,y,f)\in S_e}\sum_{t=1}^{|y|}
\mathrm{KL}\left(\pi_\theta(\cdot\mid x,y_{<t})\ \middle\Vert\ \operatorname{stopgrad}[\pi_\theta(\cdot\mid x,f,y_{<t})]\right)
$$

$$
\mathrm{KL}(p\Vert q)=\sum_{v\in V_{KL}}p(v)\log\frac{p(v)}{q(v)}
$$

Symbol definitions:

- $x$: input problem; $y_i$: initial strong-model answer; $\hat y_i$: revised answer after critique; $f_i$: weak-model critique.
- $\pi_{\theta_e}$: expert/strong policy used to sample on-policy answers; $\pi_w$: weak critic policy.
- $r_{out}$: outcome correctness indicator; $r_{rub}$: rubric usefulness indicator; $h_i$: product filter; $S_e$: retained filtered triples.
- $\theta$: trainable policy parameters; $t$: token index; $|y|$: answer length; $y_{<t}$: answer prefix before token $t$.
- $V_{KL}$: token vocabulary support for the KL computation; $p,q$: distributions in the KL definition.

Domain-specific definitions:

- O-PCD: on-policy critique distillation.
- Weak critic: lower-capability model whose critique is used only if filtered as useful.
- Critique-and-refine: generate answer, critique it, revise it, then check outcome and rubric.
- Token-level KL: distribution-matching loss over next-token distributions.

Explanation:

- Weak critiques are not accepted by default; they must pass both outcome and rubric filters.
- The retained set $S_e$ is the score-bearing selection rule for distillation data.
- The loss pushes no-critique behavior toward critic-conditioned behavior on filtered examples.
- The exact rubric classifier/judge details remain a source caveat.

### 2606.00611 — TRACE trajectory risk compression

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00611__trace-trajectory-risk-aware-compression-for-long-horizon-agent-safety|2606.00611 TRACE]]

Mechanism name: latent evidence state, unsafe probability, and BCE loss.

Source anchors: `texts/2606.00611.txt` lines 130-210, 217-324, and 326-389; review-log lines 118 and 165; audit section `2606.00611`.

Exactness label: `faithful-formalization`; appendix-level latent-swap/token-shuffle controls remain unresolved.

MathJax:

$$
\tau=(x_1,x_2,\ldots,x_L)
$$

$$
S=C_\phi(\tau)=C_\phi([E_\tau;q_1,\ldots,q_K])[-K:]
$$

$$
\tau\xrightarrow{C_\phi}S
$$

$$
Y=[E_\tau;W_{c\to r}(S)]
$$

$$
\hat p=\sigma\left(w^\top h_{end}(R_\theta(Y))\right)
$$

$$
L=-\left[y\log\hat p+(1-y)\log(1-\hat p)\right]
$$

Symbol definitions:

- $\tau$: full agent trajectory; $x_l$: trajectory element at position $l$; $L$: trajectory length.
- $E_\tau$: trajectory embedding sequence; $q_1,\ldots,q_K$: learned query tokens; $K$: number of latent evidence tokens.
- $C_\phi$: compressor with parameters $\phi$; $S$: latent evidence state from the final $K$ tokens.
- $W_{c\to r}$: projection from compressor latent state to reader input space; $Y$: reader input combining raw trajectory embedding and projected evidence state.
- $R_\theta$: reader/classifier with parameters $\theta$; $h_{end}$: terminal hidden state; $w$: classifier vector; $\sigma$: logistic sigmoid.
- $\hat p$: predicted unsafe probability; $y\in\{0,1\}$: true trajectory-risk label; $L$: binary cross-entropy loss when used as a loss symbol.

Domain-specific definitions:

- TRACE: trajectory risk-aware compression for long-horizon agent safety.
- Latent evidence state: compact learned representation of dispersed risk evidence.
- Compression-reference reader: reader that sees both raw trajectory and compressed evidence reference.
- Unsafe Recall / Safety Rate: evaluation metrics named in the source note; not rederived here.

Explanation:

- The raw trajectory is not discarded; $S$ augments it as a reference state.
- $\hat p$ is a binary unsafe-risk prediction from the reader's terminal representation.
- BCE is the paper-internal training signal for the unsafe classifier.
- The formula is safe for architecture/loss, not for unresolved appendix control details.

### 2606.00671 — AXIOM trust-first evaluation

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00671__axiom-a-trust-first-neuro-symbolic-execution-architecture-for-verifiable-mathematical-reason|2606.00671 AXIOM]]

Mechanism name: trust score and abstention-first routing.

Source anchors: `texts/2606.00671.txt` lines 46-50, 130-140, 145-204, and 230-319; audit section `2606.00671`.

Exactness label: `faithful-formalization` for the trust metric and pipeline predicate; note status caveat remains because public deployment/registry claims were not independently verified by the audit.

MathJax:

$$
\mathrm{Trust}=1-\frac{\mathrm{wrong}}{\mathrm{attempted}}
$$

$$
\operatorname{AXIOM}(T)=
\begin{cases}
(\varnothing,\mathrm{abstain}), & \neg\operatorname{route}(T) \\
(\varnothing,\mathrm{abstain}), & \operatorname{translate}(T)=\mathrm{unknown} \\
(\varnothing,\mathrm{abstain}), & \neg\operatorname{verify}(\operatorname{translate}(T)) \\
(\operatorname{answer}(T),\mathrm{verified}), & \operatorname{verify}(\operatorname{translate}(T))
\end{cases}
$$

Symbol definitions:

- $T$: mathematical task or problem instance.
- $\mathrm{wrong}$: count of attempted non-abstain answers that are wrong.
- $\mathrm{attempted}$: count of non-abstain attempts.
- $\operatorname{route}(T)$: predicate that the task can be routed to a handler.
- $\operatorname{translate}(T)$: translation of the task into a handler-verifiable form; `unknown` is explicit abstention.
- $\operatorname{verify}(\cdot)$: handler verification predicate; $\varnothing$: no answer.

Domain-specific definitions:

- Trust-first evaluation: count wrong attempted answers while excluding explicit abstentions.
- Abstain: return no answer rather than an unverifiable answer.
- CAS handler: symbolic or computational handler used to verify a translated problem.

Explanation:

- The trust metric penalizes wrong attempted answers, not explicit `unknown` abstentions.
- The pipeline is a gate: missing route, unknown translation, or failed verification all produce abstention.
- The mechanism is about verifiable answer admission, not maximizing raw answer rate.
- The audit keeps deployment and registry claims scope-limited.

### 2606.01160 — Expected Value Alignment for generative reward modeling

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01160__expected-value-alignment-for-generative-reward-modeling-in-formal-mathematics-verification|2606.01160 Expected Value Alignment]]

Mechanism name: anchor-token probability, expected score, EVA loss, and total objective.

Source anchors: `texts/2606.01160.txt` lines 21-28, 56-65, 198-267, and 271-290; audit section `2606.01160`.

Exactness label: EVA equations are source-visible; benchmark outcome claims remain medium confidence.

MathJax:

$$
z_A=[z_{v_1},z_{v_2},z_{v_3},z_{v_4},z_{v_5}]
$$

$$
P(i\mid X,Y_{<t})=\frac{\exp(z_{v_i}/\tau)}{\sum_{j\in A}\exp(z_{v_j}/\tau)},\qquad i\in A
$$

$$
E[R]=\sum_{i=1}^5 i\,P(i\mid X,Y_{<t})
$$

$$
L_{EVA}=\frac{1}{K}\sum_{k=1}^K\left(E[R_k]-R_{GT,k}\right)^2
$$

$$
L_{Total}=L_{SFT}+\alpha L_{EVA}
$$

Symbol definitions:

- $X$: input formal-math prompt or context; $Y_{<t}$: generated prefix before time $t$.
- $A$: anchor-token set for ratings; $v_i$: anchor token for score $i$; $z_{v_i}$: logit for anchor token $v_i$; $z_A$: vector of anchor logits.
- $\tau$: temperature in the anchor-token softmax; $P(i\mid X,Y_{<t})$: probability assigned to score $i$.
- $E[R]$: expected reward/score; $K$: number of supervised EVA examples; $R_{GT,k}$: ground-truth score for example $k$.
- $L_{EVA}$: expected-value alignment loss; $L_{SFT}$: supervised fine-tuning loss; $\alpha$: EVA loss weight; $L_{Total}$: combined objective.

Domain-specific definitions:

- Generative reward model: model that emits or scores reward through generated tokens rather than a separate scalar head.
- Anchor token: token representing a discrete reward level.
- Lean 4 verification: formal-math verification environment named in the source note.

Explanation:

- The mechanism maps five anchor-token logits into a probability distribution over scores.
- The expected score $E[R]$ is trained against a ground-truth scalar score.
- EVA is added to supervised fine-tuning with weight $\alpha$.
- Logic/Alignment/Clarity are source-domain dimensions, but the equation above is the score alignment mechanism.

### 2606.01351 — Entropy dynamics orchestration

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01351__recognize-your-orchestrator-an-entropy-dynamics-perspective-for-llm-multi-agent-systems|2606.01351 Entropy dynamics orchestration]]

Mechanism name: scheduler entropy and entropy-change decomposition.

Source anchors: `texts/2606.01351.txt` lines 80-169, 158-206, and 300-399; audit section `2606.01351`.

Exactness label: `interpretive-compression`; the equation is a theory-model diagnostic, not a validated universal score for every multi-agent system.

MathJax:

$$
p_i(k)=P(e_k=e_i\mid C_{k-1}),\qquad \sum_{i=1}^n p_i(k)=1
$$

$$
\bar H(t)=-\sum_{i=1}^n p_i(t)\log_2 p_i(t)
$$

$$
\frac{d\bar H}{dt}=F_{task}[\Psi_t]+D_{context}[\Psi_t]
$$

$$
D_{context}\to\frac{\beta}{t+1}
$$

$$
\bar H(t)=\int_0^t(F_{task}+D_{context})\,dt
=A_{task}e^{-\gamma t}\sin(\omega t+\phi)+\beta\ln(t+1)+H_0
$$

Symbol definitions:

- $e_i$: expert, agent, or role option; $e_k$: selected option at step $k$; $n$: number of options.
- $C_{k-1}$: context before step $k$; $p_i(k)$: scheduler probability of selecting option $i$.
- $\bar H(t)$: scheduler entropy at time $t$; $\log_2$: base-2 logarithm.
- $\Psi_t$: system state or trajectory context at time $t$.
- $F_{task}$: task-driven entropy-change functional; $D_{context}$: context-driven entropy-change functional.
- $\beta$: context drift coefficient; $A_{task}$: task oscillation amplitude; $\gamma$: decay rate; $\omega$: angular frequency; $\phi$: phase; $H_0$: initial entropy.

Domain-specific definitions:

- Scheduler entropy: uncertainty over which expert/agent/role the orchestrator will choose.
- Orchestrator: component that selects or coordinates agents in a multi-agent system.
- IWG, checkpoint validation, task success, step success, LCS-F1: evaluation terms named in the note but not expanded into new formulas here.

Explanation:

- The score-like object is entropy over orchestration choices.
- The model separates task pressure from contextual drift.
- The closed-form trajectory is a theory-model fit; use it as a diagnostic, not as a universal MAS law.
- The audit keeps note-review status and validation limits explicit.

### 2606.02438 — LLM-evolved pattern generators for optimal classical planning

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02438__llm-evolved-pattern-generators-for-optimal-classical-planning|2606.02438 LLM-evolved pattern generators]]

Mechanism name: plan cost, saturated cost partitioning, admissible heuristic scoring, and maximum single-change formula.

Source anchors: `texts/2606.02438.txt` lines 57-126; review-log lines 125 and 170; audit section `2606.02438`.

Exactness label: `faithful-formalization` for background/SCP formulas; generated-pattern evaluation remains scope-limited.

MathJax:

$$
\operatorname{cost}(\pi)=\sum_{i=1}^n\operatorname{cost}(\ell_i)
$$

$$
\sum_{i=1}^n\operatorname{cost}_i(\ell)\le \operatorname{cost}(\ell),\qquad \ell\in L(T)
$$

$$
h_{CP}(\operatorname{cost},s)=\sum_{i=1}^n h^*_{T_i}(\operatorname{cost}_i,s)
$$

$$
rem_0=cost,
\quad cost_i=saturate(h_i,rem_{i-1}),
\quad rem_i=rem_{i-1}-cost_i
$$

$$
mscf_i(\ell)=\sup_{\langle s,\ell,s'\rangle\in T(T_i)}\left(h^*_{T_i}(c,s)-h^*_{T_i}(c,s')\right)
$$

Symbol definitions:

- $\pi$: plan; $\ell_i$ or $\ell$: operator/action label; $n$: number of plan steps or partitions depending on formula.
- $T$: transition system; $L(T)$: labels/actions of $T$; $T_i$: abstraction or pattern database transition system.
- $\operatorname{cost}$: base cost function; $\operatorname{cost}_i$: partitioned cost function.
- $s,s'$: states; $h^*_{T_i}$: optimal heuristic value in abstraction $T_i$; $h_{CP}$: cost-partitioned heuristic.
- $rem_i$: remaining cost after saturated allocation; $saturate(\cdot)$: saturated cost-allocation operation; $h_i$: heuristic used in saturation.
- $mscf_i(\ell)$: maximum single-change formula for label $\ell$ under abstraction $i$; $c$: cost argument inside $h^*$.

Domain-specific definitions:

- PDB: pattern database heuristic.
- SCP: saturated cost partitioning, an admissibility-preserving allocation of action costs.
- OpenEvolve: LLM evolution framework used to generate pattern generators in the paper.
- Autoscale coverage: benchmark/evaluation setting named in the source note.

Explanation:

- The formulas define admissible heuristic scoring around the learned pattern generator.
- The generator is scored through planning coverage and admissibility-related criteria, not a direct LLM reward formula.
- The audit removed cross-paper contamination: SCP belongs here, not to Harness-1.
- Per-domain coverage table values and generated examples remain unresolved.

### 2606.02461 — AGENTCL continual-learning benchmark

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02461__agentcl-toward-rigorous-evaluation-of-continual-learning-in-language-agents|2606.02461 AGENTCL]]

Mechanism name: plasticity, stability, and generalization gains.

Source anchors: `texts/2606.02461.txt` lines 226-275; review-log lines 128 and 172; audit section `2606.02461`.

Exactness label: `interpretive-compression` for benchmark framing; PG/SG/GG formulas are source-visible.

MathJax:

$$
PG_i=F_i-B_i
$$

$$
SG_i=S_i-F_i
$$

$$
GG_j=H_j-B_j
$$

Symbol definitions:

- $i$: task index in the continual-learning stream; $j$: held-out or generalization task index.
- $B_i$: baseline score before learning for task $i$; $F_i$: first-pass score after encountering task $i$.
- $S_i$: second-pass score for task $i$ after memory accumulation; $H_j$: held-out/generalization score for task $j$.
- $PG_i$: plasticity gain; $SG_i$: stability gain; $GG_j$: generalization gain.

Domain-specific definitions:

- Continual learning: learning across a task stream without forgetting or overfitting to only the immediate task.
- Memory stream: sequence of experiences/tasks used to update or condition the agent.
- Two-pass protocol: first and second evaluation passes used to separate plasticity and stability.
- MemProbe: implementation detail named in the note; unresolved in the audit.

Explanation:

- PG measures improvement from baseline to first exposure.
- SG measures stability or retention improvement from first to later exposure.
- GG measures held-out generalization relative to baseline.
- MemProbe details and all downstream result tables remain unresolved.

### 2606.02488 — RASER recoverability-aware routing

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02488__raser-recoverability-aware-selective-escalation-router-for-multi-hop-question-answering|2606.02488 RASER]]

Mechanism name: RASER-3 cost-aware route argmax, plus RASER-2 bridgeability threshold and training label.

Source anchors: `texts/2606.02488.txt` lines 220-289 and 301-347; cleaned local extract `crates/ploke-selection-score/docs/text/2606.02488__raser.txt` lines 31-60; review-log lines 130 and 175; audit section `2606.02488`.

Exactness label: `interpretive-compression` for the learned evaluators; cost-aware argmax, bridgeability threshold, and PRUNE training label are source-supported.

MathJax:

$$
r^*=\arg\max_{r\in R}\left[\hat f_r(s)-\lambda c_r\right],\qquad
R=\{\mathrm{ONE\text{-}SHOT\ RAG},\mathrm{PRUNE},\mathrm{IRCOT}^*\}
$$

$$
\operatorname{RASER2}(s)=
\begin{cases}
\mathrm{PRUNE}, & p(\mathrm{BRIDGEABLE}\mid s)\ge \theta \\
\mathrm{ONE\text{-}SHOT\ RAG}, & p(\mathrm{BRIDGEABLE}\mid s)<\theta
\end{cases}
$$

$$
y=\mathbf 1\left[F1_{\mathrm{PRUNE}}-F1_{\mathrm{ONE\text{-}SHOT\ RAG}}>\tau\right]
$$

Symbol definitions:

- $r$: route/action choice; $r^*$: selected route.
- $R$: set of available RASER-3 routes: one-shot RAG, PRUNE, and IRCoT*.
- $s$: cheap feature state from the one-shot draft/retrieval context.
- $\hat f_r(s)$: route-specific predicted answer F1 for route $r$ given cheap feature state $s$.
- $\lambda$: cost penalty weight; $c_r$: token or compute cost of route $r$.
- $p(\mathrm{BRIDGEABLE}\mid s)$: RASER-2 classifier probability that PRUNE will recover the answer enough to justify escalation.
- $\theta$: RASER-2 escalation threshold; the paper example uses $\theta=0.20$.
- $y$: RASER-2 bridgeability training label.
- $F1_{\mathrm{PRUNE}}$ and $F1_{\mathrm{ONE\text{-}SHOT\ RAG}}$: observed answer F1 for the PRUNE route and one-shot route on training data.
- $\tau$: minimum F1 improvement used to label a question bridgeable; the paper sets $\tau=0.1$.

Domain-specific definitions:

- Recoverability-aware routing: choose escalation only when extra retrieval/reasoning is expected to improve answer quality enough.
- PRUNE and IRCoT*: more expensive retrieval/reasoning routes than one-shot RAG.
- GBM: gradient-boosted model used for the paper's routers: a binary classifier for RASER-2 and three route-specific regressors for RASER-3.

Explanation:

- RASER-3 maximizes predicted route F1 minus token cost, making $\lambda$ a cost/accuracy dial rather than a fixed universal score.
- RASER-2 uses a cheaper binary escalation threshold: run PRUNE only when $p(\mathrm{BRIDGEABLE}\mid s)$ crosses $\theta$.
- The RASER-2 label is positive only when PRUNE improves the one-shot answer by more than $\tau$ F1; this avoids treating every hard question as recoverable.
- The feature state is cheap and available after one-shot RAG; the route choice avoids unnecessary expensive retrieval.
- Token/F1 tradeoff is the score-bearing decision boundary; complete per-model/per-dataset tables and threshold-sweep details remain unresolved.

### 2606.02536 — Behavioral trait-vector diff scoring

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02536__tracking-the-behavioral-trajectories-of-adapting-agents|2606.02536 Behavioral trajectories]]

Mechanism name: normalized embedding diff, linear trait score, and validation metrics.

Source anchors: `texts/2606.02536.txt` lines 72-95 and 184-189; review-log lines 131 and 176; audit section `2606.02536`.

Exactness label: `faithful-formalization` for the scoring procedure; trusted-intermediary/HMAC deployment protocol remains lightly captured.

MathJax:

$$
\hat e=\frac{e}{\|e\|}
$$

$$
d_i=E(A_i)-E(B_i),\qquad \hat d_i=\frac{d_i}{\|d_i\|}
$$

$$
\hat y=\hat d\cdot w+b
$$

$$
\mathrm{SignAccuracy}=\frac{1}{m}\sum_{i=1}^m \mathbf 1\{\operatorname{sign}(\hat y_i)=\operatorname{sign}(y_i)\},\qquad \rho=\operatorname{Spearman}(\hat y,y)
$$

Symbol definitions:

- $e$: embedding vector; $\hat e$: normalized embedding vector; $\|e\|$: vector norm.
- $B_i$: before-edit artifact for pair $i$; $A_i$: after-edit artifact for pair $i$; $E(\cdot)$: embedding function.
- $d_i$: before/after embedding difference; $\hat d_i$: normalized difference vector.
- $w$: learned trait vector weights; $b$: intercept; $\hat y$: predicted trait-change score; $y_i$: labeled trait-change target.
- $m$: number of labeled validation pairs; $\rho$: Spearman rank correlation.

Domain-specific definitions:

- Skill diff: before/after change to an agent skill or source artifact.
- Trait vector: learned direction in embedding space corresponding to a behavior trait.
- Ridge regression: regularized linear regression used to fit $w,b$.
- LOOCV/PRESS: validation protocol named in the note.

Explanation:

- The score is a learned projection of normalized before/after text-embedding differences.
- Sign accuracy checks direction of change; Spearman $\rho$ checks rank agreement.
- The source-reported validation result is 91.2% sign accuracy and $\rho=0.82$ under LOOCV.
- The broader trusted-intermediary protocol is not formalized here beyond the scoring core.

## 2. Formal decision logic and benchmark protocols

These mechanisms are source-supported but are not full scalar objective formulas. They are separated from the high-confidence equation bank to avoid over-formalization.

### 2606.00103 — Interactive executable-game benchmark

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00103__evaluating-interactive-reasoning-in-large-language-models-a-hierarchical-benchmark-with-exec|2606.00103 Interactive executable games]]

Mechanism name: episode status logic, success rate, average turns, and efficiency.

Source anchors: `texts/2606.00103.txt` lines 133-232; review-log lines 114 and 161; audit section `2606.00103`.

Exactness label: `interpretive-compression`; status logic is visible, while metric equations are benchmark-metric reconstructions from source prose.

MathJax:

$$
\mathrm{status}(e,t)=
\begin{cases}
\mathrm{FormatError}, & \neg\operatorname{valid}(a_t) \\
\mathrm{Success}, & a_t=\operatorname{submit}(\hat y_t)\land E.checkAnswer(\hat y_t) \\
\mathrm{Failure}, & a_t=\operatorname{submit}(\hat y_t)\land \neg E.checkAnswer(\hat y_t) \\
\mathrm{Continue}, & a_t=\operatorname{query}(q_t)\land t<T_{max} \\
\mathrm{Timeout}, & t=T_{max}\land \neg\mathrm{Success}
\end{cases}
$$

$$
\mathrm{SuccessRate}=\frac{\#\mathrm{Success}}{\#\mathrm{Episodes}}
$$

$$
\mathrm{AvgTurns}=\frac{1}{|\mathcal S|}\sum_{e\in\mathcal S}N_e,\qquad
\mathrm{Efficiency}=\frac{\mathrm{SuccessRate}}{\mathrm{AvgTurns}}
$$

Symbol definitions:

- $e$: benchmark episode; $t$: turn index; $a_t$: action at turn $t$.
- $E$: executable game environment; $E.checkAnswer$: environment answer checker.
- $\hat y_t$: submitted answer; $q_t$: query; $T_{max}$: maximum turns.
- $\mathcal S$: set of successful episodes when used for average turns; $N_e$: number of turns in episode $e$.

Domain-specific definitions:

- Executable game: benchmark environment where the model interacts by structured query/submit actions.
- FormatError: invalid action format status.
- Contextual robustness / counterfactual revision: perturbation layers named in the note; not expanded into formulas here.

Explanation:

- The episode logic scores interaction outcomes, not just final answer text.
- Success, failure, format error, and timeout are distinct status outcomes.
- Efficiency compresses success and interaction cost.
- Per-category and perturbation-specific formulas remain unresolved.

### 2606.00384 — VESTA metric-selected statistical model revision

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00384__vesta-visual-exploration-with-statistical-tool-agents|2606.00384 VESTA]]

Mechanism name: metric-directed model selection over candidate statistical models.

Source anchors: `texts/2606.00384.txt` lines 178-252, 304-345, and 360-501; review-log lines 116 and 163; audit section `2606.00384`.

Exactness label: `faithful-formalization` for the algorithmic loop; only selection rules are included because AIC/JSD/ELPD-LOO formulas were not transcribed in the notes.

MathJax:

$$
M_{t+1}=\arg\min_{M\in\mathcal M_t}R(M;D)\quad\text{when }R\in\{\mathrm{AIC},\mathrm{JSD}\}
$$

$$
M_{t+1}=\arg\max_{M\in\mathcal M_t}\mathrm{ELPD\text{-}LOO}(M;D)
$$

Symbol definitions:

- $D$: dataset; $M$: candidate probabilistic model; $\mathcal M_t$: candidate model set at iteration $t$.
- $M_{t+1}$: selected next model; $R(M;D)$: metric value for model $M$ on data $D$.
- AIC: Akaike information criterion; JSD: Jensen-Shannon divergence; ELPD-LOO: expected log predictive density under leave-one-out evaluation.

Domain-specific definitions:

- Statistical tool agent: agent that creates/selects visual diagnostic tools for model refinement.
- DAWN: source benchmark/evaluation setting in the note.
- Generated visual diagnostic tool: a tool created to inspect or revise statistical models.

Explanation:

- The formal score is metric-directed selection among models.
- AIC/JSD are minimized; ELPD-LOO is maximized.
- The page does not reconstruct untranscribed AIC/JSD/ELPD formulas or Figure 4 numeric plot values.
- Treat this as a model-revision rule, not a full derivation of all DAWN metrics.

### 2606.02373 — Harness-1 state-externalizing search harness

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02373__harness-1-reinforcement-learning-for-search-agents-with-state-externalizing-harnesses|2606.02373 Harness-1]]

Mechanism name: retrieval reward-component set and authority-bounded state evaluation.

Source anchors: `texts/2606.02373.txt` lines 204-219 and 224-305; review-log lines 124, 152, and 169; audit section `2606.02373`.

Exactness label: `interpretive-compression`; exact CISPO RL objective and full tables are unresolved.

MathJax:

$$
\mathcal R_{H1}(\tau)=\{R_{set},R_{ansdoc},R_{trajrel},R_{trajans},D_{tool},B_{found},P_{turn}\}
$$

$$
\operatorname{keep}(d)=\operatorname{relevant}(d)\land\operatorname{evidenceLinked}(d)
$$

$$
\operatorname{stop}(\tau)=\operatorname{answerFound}(\tau)\lor \operatorname{budgetExhausted}(\tau)
$$

Symbol definitions:

- $\tau$: search trajectory; $d$: retrieved document or evidence item.
- $\mathcal R_{H1}$: source-supported set of reward components, not a weighted sum formula.
- $R_{set}$: set-level curated recall component; $R_{ansdoc}$: answer-document recall component.
- $R_{trajrel}$: trajectory-level relevant-document recall; $R_{trajans}$: trajectory-level answer-document recall.
- $D_{tool}$: tool-diversity component; $B_{found}$: answer-found bonus; $P_{turn}$: turn penalty.

Domain-specific definitions:

- State-externalizing harness: environment that manages working memory, curated sets, evidence links, and verification records outside the policy.
- Curated recall: recall over documents retained in the harness state.
- Authority boundary: separation between policy decisions and harness-managed state verification.

Explanation:

- The source supports reward components, not a fully transcribed RL objective.
- $\mathcal R_{H1}$ is intentionally a set of components rather than a scalar sum with invented weights.
- The keep/stop predicates formalize state-management logic supported by the note.
- Saturated cost partitioning is not part of Harness-1; that contamination was corrected in the review log.

### 2606.02449 — HLL human-verification benchmark

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02449__hll-can-agents-cross-humanity-s-last-line-of-verification|2606.02449 HLL]]

Mechanism name: trace-conditioned validation over human-verification tasks.

Source anchors: `texts/2606.02449.txt` lines 20-34, 41-65, 100-149, and 158-163; review-log lines 126 and 171; audit section `2606.02449`.

Exactness label: `interpretive-compression`; per-CAPTCHA-family scoring details are unresolved.

MathJax:

$$
\operatorname{HLLSuccess}(e)=
\operatorname{TaskSolved}(e)\land\operatorname{TraceValid}(e)\land\operatorname{BarrierCrossed}(e)
$$

$$
\mathrm{PassRate}_{HLL}=\frac{|\{e\in\mathcal E:\operatorname{HLLSuccess}(e)\}|}{|\mathcal E|}
$$

Symbol definitions:

- $e$: HLL benchmark episode; $\mathcal E$: set of HLL episodes.
- $\operatorname{TaskSolved}$: final task-success predicate.
- $\operatorname{TraceValid}$: trace-conditioned dynamic validation predicate.
- $\operatorname{BarrierCrossed}$: predicate that the human-verification boundary was crossed.
- $\mathrm{PassRate}_{HLL}$: pass rate under the HLL success predicate.

Domain-specific definitions:

- HLL: Humanity's Last Line, benchmark around deliberate human-verification barriers.
- CAPTCHA-family benchmark: task family testing human-verification boundaries.
- Trace-conditioned validation: validating the process, not only the final answer.

Explanation:

- The mechanism scores whether an agent can cross a human-verification barrier through a valid trace.
- This is an authority-boundary benchmark, not merely image recognition.
- The pass-rate formula is a benchmark wrapper around source-supported success predicates.
- Individual CAPTCHA-family scoring details are not reconstructed.

### 2606.02470 — MCP-Persona personalized tool benchmark

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02470__mcp-persona-benchmarking-llm-agents-on-real-world-personal-applications-via-environment-simu|2606.02470 MCP-Persona]]

Mechanism name: execution scoring and simulation-fidelity predicate.

Source anchors: `texts/2606.02470.txt` lines 114-118, 130-144, 450-480, 492-523, and 525-552; review-log lines 128, 129, and 173; audit section `2606.02470`.

Exactness label: `interpretive-compression`; full pipeline construction algorithms remain unresolved.

MathJax:

$$
\operatorname{MCPPersonaSuccess}(\tau)=
\operatorname{ExecSuccess}(\tau)\land\operatorname{PersonaConsistent}(\tau)\land\operatorname{SimFidelityOK}(\tau)
$$

$$
\mathrm{TaskSuccess} = \frac{|\{\tau\in\mathcal T:\operatorname{MCPPersonaSuccess}(\tau)\}|}{|\mathcal T|}
$$

Symbol definitions:

- $\tau$: agent execution trace for a personalized tool task; $\mathcal T$: task set.
- $\operatorname{ExecSuccess}$: execution-scoring predicate for whether the task was completed.
- $\operatorname{PersonaConsistent}$: predicate that the action remains consistent with the generated/user persona context.
- $\operatorname{SimFidelityOK}$: predicate that the simulated environment has acceptable fidelity for the benchmark.
- $\mathrm{TaskSuccess}$: task success rate over the benchmark tasks.

Domain-specific definitions:

- MCP: Model Context Protocol tool environment in the paper's benchmark setting.
- Tool-Traverse, Context-Tree, Persona-Gen: construction components named in the note.
- Simulation fidelity: whether simulated tools/personas remain realistic enough for benchmark claims.

Explanation:

- The paper's score-bearing object is personalized tool execution under simulated account/context constraints.
- Human verification and simulation fidelity matter alongside raw execution success.
- The predicates avoid inventing hidden weights or per-tool scoring formulas.
- Full construction algorithm details remain unresolved.

### 2606.02484 — Iteris agentic computational-math loop

Paper: [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02484__iteris-agentic-research-loops-for-computational-mathematics|2606.02484 Iteris]]

Mechanism name: verified mathematical-output acceptance in an explore-plan-execute-review loop.

Source anchors: `texts/2606.02484.txt` lines 31-36, 59-83, 89-95, and 143-204; review-log lines 129 and 174; audit section `2606.02484`.

Exactness label: `interpretive-compression`; theorem appendices were not independently reconstructed or re-proved.

MathJax:

$$
\mathcal O_{Iteris}=\{\mathrm{phase\ diagram},\mathrm{QRCP\ counterexample\ family},\mathrm{verified\ proof\ artifact}\}
$$

$$
\operatorname{Accept}(o)=o\in\mathcal O_{Iteris}\land\operatorname{HumanVerified}(o)
$$

$$
\operatorname{LoopState}_{t+1}=\operatorname{Review}(\operatorname{Execute}(\operatorname{Plan}(\operatorname{Explore}(\operatorname{LoopState}_t))))
$$

Symbol definitions:

- $o$: candidate mathematical output artifact.
- $\mathcal O_{Iteris}$: source-supported classes of mathematical outputs named in the note.
- $\operatorname{HumanVerified}$: human verification predicate for final correctness.
- $\operatorname{LoopState}_t$: file/project state at loop step $t$; Explore/Plan/Execute/Review: source-supported roles or phases.

Domain-specific definitions:

- QRCP: QR with column pivoting; source domain for the counterexample family.
- File-state substrate: persistent project state used by the loop.
- Agentic research loop: iterative explore-plan-execute-review workflow for computational mathematics.

Explanation:

- The accepted score is verified mathematical output, not autonomous proof certainty.
- Human verification remains part of the acceptance predicate.
- The loop equation is phase structure, not a performance objective.
- Theorem appendices were not re-proved by the audit.

## Separate unresolved and candidate mechanisms

The following mechanisms are source leads or paper-internal evaluations, but the current note/text state does not justify final equations. They should remain separate from the equation bank until repaired by a later deep read.

### Prose-only or not-safely-formalizable mechanisms

| Paper | Current status | Why no final formula appears here |
| --- | --- | --- |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00005__emergent-collaborative-deliberation-in-multi-model-ai-systems-a-bft-derived-protocol-for-epi|2606.00005]] | prose-only | Deliberation sessions, evidence retrieval, validation, and bias deltas are described, but no explicit equation or section anchor is approved. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00376__the-deterministic-horizon-when-extended-reasoning-fails-and-tool-delegation-becomes-necessar|2606.00376]] | not-safely-formalizable | Accuracy, State-Space Jaccard, and recovery metrics exist, but theorem/proof and $d^*$ derivation are unreconstructed. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00476__doing-what-they-say-not-what-they-reason-locating-the-faithfulness-gap-in-llm-agents|2606.00476]] | not-safely-formalizable | Conclusion→action and reasoning→conclusion decomposition is source-visible, but parser/prompt variants behind reported artifacts are unresolved. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00618__efficient-test-time-inference-for-generative-planning-models|2606.00618]] | not-safely-formalizable | OCL/OCLGen setup and metrics are visible, but depth-partitioned selection and heuristic pseudocode are missing. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00642__hidden-thoughts-are-not-secret-reasoning-trace-exposure-in-llms|2606.00642]] | prose-only | Structural validity, exposure fidelity, behavior preservation, and utility are not exposed as exact equations in the current note. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00708__mosaic-modular-orchestration-for-structured-agentic-intelligence-and-composition|2606.00708]] | prose-only | Predictive accuracy, distributional fidelity, execution reliability, and risk/tail behavior lack exact scoring formulas in the note. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00756__comic-collaborative-memory-and-insights-circulation-for-long-horizon-llm-agents-in-cloud-edg|2606.00756]] | prose-only | Progress, grounding, success, and token bounds are named but not formalized. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00765__falat-tracing-failures-in-llm-agent-trajectories-via-dependency-guided-search|2606.00765]] | prose-only | Failure-attribution accuracy and decisive-step accuracy are source leads without an approved formula. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00914__adversarial-feeds-steer-llm-agent-decisions-against-their-defaults|2606.00914]] | prose-only | Adversarial capitulation, saturation/asymmetry, dose-response, and mitigation effects are not represented as exact equations. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01120__diagnosing-llm-arbitration-behavior-over-pre-evidence-epistemic-states-in-rag-based-fact-che|2606.01120]] | prose-only | Persistence/correction percentages and final-verdict accuracy lack exact denominators in the note. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01139__skillrevise-improving-llm-authored-agent-skills-via-trace-conditioned-skill-revision|2606.01139]] | prose-only | Empirical utility from re-execution and success-rate improvements lack an exact selection rule. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01185__skill-issues-data-centric-optimization-of-lakehouse-agents|2606.01185]] | prose-only | Trace-level signals and programmatic checks are visible only at high level. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01199__can-llm-agents-sustain-long-horizon-organizational-dynamics|2606.01199]] | prose-only | Organizational coherence and execution grounding are not formula-approved. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01230__homeflow-a-data-flywheel-for-smart-home-agent-training-with-verifiable-simulation|2606.01230]] | prose-only | Verifiable simulation and benchmark success are source leads, but no exact reward formula is approved. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01279__andes-agent-native-data-evolving-synthesis-tool-for-autonomous-instruction-alignment|2606.01279]] | prose-only | PostTrainBench and alignment leap claims lack an exact evaluation formula in the note. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01365__early-diagnosis-of-wasted-computation-in-multi-agent-llm-systems-via-failure-aware-observabi|2606.01365]] | prose-only | Failure rates and cached LLM-judge grounding audit are not exact online-signal formulas. |

### Candidate-only mechanisms awaiting deep read or note repair

| Paper | Candidate scoring lead | Current caveat |
| --- | --- | --- |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00718__llm-driven-co-evolutionary-automated-heuristic-design-for-bi-component-coupled-combinatorial|2606.00718]] | paired execution reward, individual operator scores, pairwise synergy | Exact objective not transcribed. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00726__latent-reward-steering-an-adaptive-inference-time-framework-that-implicitly-promotes-cogniti|2606.00726]] | latent reward score, confidence gate, correctness | No formula in note. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01066__before-the-model-learns-the-bug-fuzzing-rlvr-verifiers|2606.01066]] | FPR, FNR, differential disagreement, exploit-candidate rate, reward-correctness gap, exploit rate | Exact metric definitions need deep-read line anchors. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01314__skillsmith-co-evolving-skills-and-tools-for-self-improving-agent-systems|2606.01314]] | ecological utility model, interaction matrix | Exact utility model not transcribed. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01912__smh-bench-benchmarking-llm-agents-for-environment-grounded-reasoning-and-action-in-smart-hom|2606.01912]] | task success over 1,100 smart-home tasks | First pass pending. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01991__safemcp-proactive-power-regulation-for-llm-agent-defense-via-environment-grounded-look-ahead|2606.01991]] | dual verifiable rewards, utility-vs-safety tradeoff | First pass pending; no reward formula. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02054__emot-evolving-memory-of-thought-via-symbolic-anchoring-and-memory-corrosion|2606.02054]] | accuracy, solution consistency, memory corrosion | First pass pending; no formula. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02060__where-do-deep-research-agents-go-wrong-span-level-error-localization-in-agent-trajectories|2606.02060]] | TELBench span localization and first-error accuracy | First pass pending. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02109__badger-bridging-agentic-and-deterministic-evaluation-for-generative-enterprise-reasoning|2606.02109]] | Hybrid-EX, kappa, balanced accuracy | First pass pending. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02132__learning-when-not-to-act-mitigating-tool-abuse-in-agentic-reinforcement-learning|2606.02132]] | accuracy-efficiency and fewer tool calls | First pass pending; no formula. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02282__poirot-interrogating-agents-for-failure-detection-in-multi-agent-systems|2606.02282]] | benchmark against evaluator baselines, odds ratio | First pass pending; formula unnecessary. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02355__siri-self-internalizing-reinforcement-learning-with-intrinsic-skills-for-llm-agent-training|2606.02355]] | paired skill/free rollouts and utility signals | First pass pending; utility signals not exact. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02359__moc-multi-order-communication-in-llm-based-multi-agent-systems|2606.02359]] | task performance and communication cost | First pass pending. |
| [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02372__comap-co-evolving-world-models-and-agent-policies-for-llm-agents|2606.02372]] | planning/tool-use benchmark performance, +16.75% relative improvement | First pass pending. |

### Negative or excluded from this formal document

These papers should stay out of the formal equation section unless a later note repair supplies source-visible mechanisms: 2606.00002, 2606.00288, 2606.00672, 2606.01046, 2606.01230 as background/unresolved, 2606.01386, 2606.01417, 2606.01435, 2606.01441, 2606.01444, 2606.01462, 2606.01473, 2606.01528, 2606.01552, 2606.01561, 2606.01610, 2606.01640, 2606.01725, 2606.01737, 2606.01755, 2606.01789, 2606.01810, 2606.01830, 2606.01850, 2606.01869, 2606.01886, 2606.01897, 2606.01906, 2606.01929, 2606.02048, 2606.02253, and 2606.02458.

## Consolidated symbol glossary

| Symbol | Consolidated definition |
| --- | --- |
| $a$ | Agent; in 2606.00007 reputation formulas. |
| $A_i$ | After-edit artifact in 2606.02536; also an advantage symbol with superscript in 2606.00251. |
| $A$ | Anchor-token set in 2606.01160. |
| $a_i^*$ | Reference answer for CSA query $i$. |
| $\hat a_i^{(k)}$ | CSA answer produced by probe rollout $k$. |
| $a_t$ | Action at interactive benchmark turn $t$. |
| $\alpha_a,\beta_a$ | Beta reputation counts for agent $a$. |
| $\alpha$ | EVA loss weight in 2606.01160. |
| $B_i$ | Baseline score in AGENTCL or before-edit artifact in trait-vector scoring. |
| $B$ | Benign/broken group in sanction FPR, or minibatch in GRPO depending on local section. |
| $b$ | Linear-regression intercept in trait-vector scoring. |
| $\beta$ | KL penalty in GRPO or context-drift coefficient in entropy dynamics; defined locally where used. |
| $c$ | Contribution/artifact in 2606.00007; cost argument in 2606.02438 where local context says so. |
| $C_{ij}$ | Normalized local trust matrix entry. |
| $C_{k-1}$ | Context before orchestrator step $k$. |
| $C_\phi$ | TRACE compressor with parameters $\phi$. |
| $c_r$ | Cost of RASER route $r$. |
| $D$ | Dataset in VESTA or CSA source dataset, local context determines meaning. |
| $D_{context}$ | Context-driven entropy-change functional. |
| $D_{div}$ | CSA diversity-filtered subset. |
| $D_{tool}$ | Harness-1 tool-diversity reward component. |
| $d_i,\hat d_i$ | Embedding difference and normalized difference in trait-vector scoring. |
| $\delta$ | Reputation decay rate. |
| $e,e_i,e_k$ | Embedding vector in trait-vector scoring or expert/role option in entropy dynamics; local context disambiguates. |
| $E$ | Executable game environment or embedding function; local section defines it. |
| $E_\tau$ | TRACE trajectory embedding sequence. |
| $\epsilon$ | GRPO clipping parameter or EigenTrust prior-mixing parameter; local context disambiguates. |
| $f_i$ | Weak critique in O-PCD. |
| $F_i$ | AGENTCL first-pass task score. |
| $F_{task}$ | Task-driven entropy-change functional. |
| $G$ | GRPO group size. |
| $g(\cdot)$ | CSA aggregation function over probe correctness indicators. |
| $H$ | Honest agent group in sanction FPR. |
| $H_j$ | AGENTCL held-out/generalization score. |
| $\bar H(t)$ | Scheduler entropy at time $t$. |
| $h_i$ | O-PCD filter indicator or heuristic in SCP; defined locally. |
| $h_{end}$ | Reader terminal hidden state in TRACE. |
| $h^*_{T_i}$ | Optimal heuristic value in abstraction $T_i$. |
| $K$ | Number of CSA probes, TRACE latent query tokens, or EVA examples depending on local section. |
| $L$ | Trajectory length in TRACE or loss symbol; local section defines it. |
| $\ell,\ell_i$ | Action/operator label in planning formulas. |
| $\lambda$ | RASER cost penalty weight. |
| $M,\mathcal M_t$ | Candidate model and candidate model set in VESTA. |
| $m$ | Number of trait-vector validation pairs. |
| $N$ | Number of SFT examples or episode turns depending on local section. |
| $o,o_i$ | Candidate mathematical output in Iteris or model output in CSA. |
| $p_i(k),p_i(t)$ | Orchestrator probability of selecting option $i$. |
| $\pi$ | Plan in planning formulas or policy distribution in learning formulas; local context disambiguates. |
| $\pi_\theta,\pi_{ref},\pi_w,\pi_{\theta_e}$ | Trainable policy, reference policy, weak critic policy, and expert/strong policy. |
| $q(c),q_t,q_k$ | Artifact quality, interactive query, or TRACE query token; local context defines it. |
| $R$ | Route set in RASER or metric function in VESTA; local context defines it. |
| $R_i^{(g)}$ | CSA rollout reward. |
| $r(a),r_i$ | Reputation score. |
| $r,r^*$ | RASER route and selected route. |
| $r_{out},r_{rub}$ | O-PCD outcome-correctness and rubric-usefulness indicators. |
| $\rho_i^{(g)}$ | GRPO policy ratio. |
| $\rho$ | Spearman rank correlation in trait-vector validation. |
| $S$ | TRACE latent evidence state. |
| $S_e$ | O-PCD retained filtered triple set. |
| $s,s'$ | State in planning formulas; $s$ is also RASER feature state in that section. |
| $s_{ij}$ | Local trust/satisfaction score from agent $i$ to $j$. |
| $\sigma(a),\sigma_2$ | Sanction score and Tier-2 sanction threshold. |
| $T,T_i$ | Task in AXIOM or transition system/abstraction in planning formulas; local section defines it. |
| $T_{max}$ | Maximum episode turns in the interactive benchmark. |
| $t,t_0,\tau_{last},t_{fast}$ | Time variables: current/reference/last-update/fast-track window depending on formula. |
| $\tau$ | TRACE trajectory, Harness-1 search trajectory, or MCP-Persona execution trace depending on section. |
| $\theta,\theta_{old},\phi$ | Trainable parameters, old policy parameters, and compressor parameters. |
| $v_i,z_{v_i},z_A$ | EVA anchor token, anchor-token logit, and anchor-logit vector. |
| $V_{KL}$ | Vocabulary support for KL computation. |
| $w,w_i$ | Classifier/trait vector or voting weight; local context disambiguates. |
| $W_{c\to r}$ | TRACE projection from compressor space to reader space. |
| $X,Y,Y_{<t}$ | EVA input, generation, and generation prefix; $Y$ also TRACE reader input where local context says so. |
| $x,x_i,x_l$ | Input problem, query, or trajectory element depending on section. |
| $y_i,\hat y_i,y$ | Target label, predicted/revised label or answer, and binary/trait target depending on local section. |
| $\hat p$ | TRACE predicted unsafe probability. |

## References

Primary synthesis and review sources:

- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-equation-audit|Formal scoring equation audit]] — source-anchored equation/candidate audit used as the parent artifact for this page.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/scoring-mechanisms|Scoring mechanisms]] — paper-internal scoring synthesis.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/symbolic-theses|Symbolic theses]] — symbolic exactness and caveat synthesis.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/category-map|Category map]] — campaign taxonomy context.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/review/review-log|Review log]] — final gate, caveats, and repair decisions.

Included equation or formal-logic paper notes:

- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00007__deliberative-curation-a-protocol-for-multi-agent-knowledge-bases|2606.00007]] — `texts/2606.00007.txt` lines 273-340, 384-412, 817-936.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00103__evaluating-interactive-reasoning-in-large-language-models-a-hierarchical-benchmark-with-exec|2606.00103]] — `texts/2606.00103.txt` lines 133-232.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00251__capability-self-assessment-teaching-llms-to-know-their-limits|2606.00251]] — `texts/2606.00251.txt` lines 145-315, 342-390, 499-577.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00384__vesta-visual-exploration-with-statistical-tool-agents|2606.00384]] — `texts/2606.00384.txt` lines 178-252, 304-345, 360-501.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00424__weak-critics-make-strong-learners-on-policy-critique-distillation-for-scalable-oversight|2606.00424]] — `texts/2606.00424.txt` lines 313-396.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00611__trace-trajectory-risk-aware-compression-for-long-horizon-agent-safety|2606.00611]] — `texts/2606.00611.txt` lines 130-210, 217-324, 326-389.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.00671__axiom-a-trust-first-neuro-symbolic-execution-architecture-for-verifiable-mathematical-reason|2606.00671]] — `texts/2606.00671.txt` lines 46-50, 130-140, 145-204, 230-319.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01160__expected-value-alignment-for-generative-reward-modeling-in-formal-mathematics-verification|2606.01160]] — `texts/2606.01160.txt` lines 21-28, 56-65, 198-267, 271-290.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.01351__recognize-your-orchestrator-an-entropy-dynamics-perspective-for-llm-multi-agent-systems|2606.01351]] — `texts/2606.01351.txt` lines 80-169, 158-206, 300-399.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02373__harness-1-reinforcement-learning-for-search-agents-with-state-externalizing-harnesses|2606.02373]] — `texts/2606.02373.txt` lines 204-219, 224-305.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02438__llm-evolved-pattern-generators-for-optimal-classical-planning|2606.02438]] — `texts/2606.02438.txt` lines 57-126.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02449__hll-can-agents-cross-humanity-s-last-line-of-verification|2606.02449]] — `texts/2606.02449.txt` lines 20-34, 41-65, 100-149, 158-163.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02461__agentcl-toward-rigorous-evaluation-of-continual-learning-in-language-agents|2606.02461]] — `texts/2606.02461.txt` lines 226-275.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02470__mcp-persona-benchmarking-llm-agents-on-real-world-personal-applications-via-environment-simu|2606.02470]] — `texts/2606.02470.txt` lines 114-118, 130-144, 450-480, 492-523, 525-552.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02484__iteris-agentic-research-loops-for-computational-mathematics|2606.02484]] — `texts/2606.02484.txt` lines 31-36, 59-83, 89-95, 143-204.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02488__raser-recoverability-aware-selective-escalation-router-for-multi-hop-question-answering|2606.02488]] — `texts/2606.02488.txt` lines 220-289, 301-347.
- [[queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/2606.02536__tracking-the-behavioral-trajectories-of-adapting-agents|2606.02536]] — `texts/2606.02536.txt` lines 72-95, 184-189.

Related durable concept links:

- [[agent-evaluation-verifiers]]
- [[agent-search-planning-rollouts]]
- [[agent-trajectory-failure-localization]]
- [[agent-memory-state-freshness]]
- [[agent-skill-harness-evolution]]
- [[multi-agent-governance]]
- [[tool-use-authority-safety]]
- [[self-evolving-research-systems]]
- [[knowledge-data-evolution]]
