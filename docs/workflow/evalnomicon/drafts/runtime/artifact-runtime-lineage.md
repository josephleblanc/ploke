Notes on graphing runtime-artifact lineage

Yes we might want to specify this down in our planning docs soon actually. Help
me think through this. Its kind of weird. This is looking forward a bit to
after we move past the simple "edit this text file that is being included in a
binary via `include_str!`" which is an artifical boundary for the sake of
keeping the initial prototype simple. At some point we are going to want to
turn our editing/patch generation function into a trait with an agnostic
backend and hook up our ploke-tui harness to it as the editing process.

Because the Runtime is a coding agent harness, it has the ability to:
- process a target Artifact into a code graph
- search over that code graph
- generate a patch to be applied to that code graph

Where the "code graph" here is just a View(Artifact) or Surface(Artifact) or
something similar, speaking loosely. The important thing is that the Runtime
operates over the Artifact.

So for some Runtime R1, in our initial graph with one root node, there is only
A1 to operate over.

R1(A1) -> P(1, 1) where P is a patch generated from R over A.

Then the patch is applied: P(1, 1): A1 -> A2

Because the process of generating a patch is itself under the hood an LLM call,
which is inherently nondeterminstic, we might want to write, for some indexed
attempts i, j

P_i(1, 1) =/= P_j(1, 1)

Therefore, it may be more accurate to say: P_1(1, 1): A1 -> A2 P_2(1, 1): A1 ->
A3 P_3(1, 1): A1 -> A4

Such that patch generation is a probabalistic function.

Then, returning to our loop, each Artifact A2-4 is "hydrated" or "actualized"
or whatever into R2-4, and generates their self-evaluations over some oracle O,
such that their self-evaluation is grounded in an external, non-editable target
task and evaluator. Since each artifact is editable and the code within that
allows for the oracle O to even have R as a valid target, i.e. in this case
oracle O is the multi-swe-bench and expects the evaluated runtime to have some
process to accept a jsonl, operate over a target git repostiory, and generate a
valid patch jsonl which it then runs through its MBE. In addition to this
evaluation by O, the runtime has some internally defined metrics that are
derived from the task O demands such as total token cost, tool failure rate,
etc etc.

So this "self-evaluation" then produces some records H2, H3, H4, and let us
suppose I forgot to mention the initial baseline record-forming step at startup
for A1 so we also have H1.

That lets R1 evaluate over the records with some policy function L1, L: (H_1,
.., H_i) -> H_n where H_n is an element in { H_1, .., H_i } To select the next
successor.

So more specifically here, let us suppose L1 selects H3, though it could be
any: L1(H2, H3, H4) -> H3 Since H3 corresponds to A3, we have our next
successor target.

Now we need to start thinking of a "location" in a sense. Because so far, we
have thought of a static starting point and then following generations of A2-4,
but have not considered R1 to be "at" a position, in a sense. However, now that
we want to discuss the successor handing over, it becomes germane to consider
this.

When we generated R2, we must have done it in some "place", and that "place" is
where A2 exists. And A2 can be said to "exist", in a practical sense, in the
worktree we created upon applying it's patch. But to stay very grounded here,
when we just "apply" the patch, all we did was edit some files in a worktree,
but those files were still uncommitted, so A2 could exist but could not be
recovered if we force-delete the branch. This is important, because if we apply
the patch and then have A2, and hydrate into some binary that we copy elsewhere
and run, then we still lose A2 even if we have R2. We could consider the
binaries here and say a binary is B2, and that A2 may exist without B2 and B2's
existence implies A2 existed at some point but may or may not exist etc etc,
but B2 is more of an incidental detail for us right now, and we are more
focused on R2.

So, supposing we applied the patch from R1 to A1, generating A2, then delete
the worktree, we now have no way to regain A2 again, unless by chance the
probabalistic function of patch generation happens to generate exactly the same
target artifact. Theoretically possible, perhaps, but vanishingly unlikely in
practice, at least so I suspect, but that is merely an assumption.

That means we must commit A2 on the worktree for it to really "exist" in our
graph, otherwise it is an ephemeral node that cannot be visited again, cannot
be made into a parent over its own artifact, and essentially becomes lost to us
in most useful senses. If we still have some binary B2, then we can still run
it as R2, but we have no mapping B2 -> A2.

The somewhat interesting thing that emerges from this consideration, is that
even if we lost A2, and still have B2/R2, then R2 could still generate patches,
just not for its own generating artifact. For example, we could do: R2(A1) ->
P(2, 1) P(2, 1) -> A5

This means that when we consider a location in the tree, lineage can be more
complex. In a sense, this is closer to a generative process like
self-fertilization and cross-fertilization.

So when we talk about the "location" in a graph, we are really referring to the
co-ordinate of the parents (A_i, R_j) where i == j or i =/= j

This makes things more complex, because we can no longer derive the "location"
of a node in the overall generative graph from the git tree alone. In order to
know "where" a newly generated Artifact is positioned in the graph, we
essentially need to know what its operative generator R and derivation source A
were in order to locate it in the graph, and that graph can be modeled as the
worktree with respect to A, but not with respect to R. Therefore we need an
additional mechanism to track the lineage of generative operators.
