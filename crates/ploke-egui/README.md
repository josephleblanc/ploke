Stream of consciousness readme

This is fairly off-the-cuff but is accurate and represents the kind of core model for this egui crate.

Yes that is basically what I'm thinking of, but the thing is that the Artifact is also bound to
  a location on the Tree, like the git tree (but with an abstracted backend but tehe semantics
  must basically be the same no matter the backend).

  The thing that a node is must be defined also by its lineage, both the lineage of the
  combination of Patches, where:

  A -> R
  R(A) -> P(R, A)
  P(R, A) -> A'
  A' -> R'
  ...

  The tough part is that this lineage, let's call it the ancestry so lineage doesn't get confused,
  this ancestry is not simple in a world of composable patches P, such that:
  P(R_i, A_j) -> A_{i, j} or something, the annotation is rough
  but the point is that i =/= j is valid, and i == j is valid.

  Furthermore,
  P(R_i, A_j) -> A_{i, j}
  P(R_k, A_j) -> A_{k, j}
  But also it is possible where P1 = P(R_i, A_j) is disjoint with P2 = P(R_k, A_j), s.t.
  P1 of P2(R_i, A_j) -> A_{?, j}

  basically a merge.

  It's even more complex because we might use an llm operation to resolve the conflicts, and that
  llm is itself being run by possibly a third runtime. Further, A_j is used both times above, but
  it could also be differen artifacts.

  So what i'm trying to say is that ancestry is not two-dimensional. Its more like bacteriaphages
  and prokaryotes or something. its weird.

  Anyway, what I mean to say is taht even just the ancestry is not something that can be cleanly
  represented as a graph.

  Then you add in teh git tree, with revsiitng past nodes, and the lineage of Parent<Ruler>
  Runtimes, and suddenly things get complicated.

  So basically, the first view wants to be simpler. Because we can't really represent a hypergraph
  in two, or even three, dimensions. And at least from the perspective of a git tree it looks like
  a DAG, kind of. I mean it's not. But it kind of looks like one.

  So the initial view is of the artifact tree, and the idea is that you can step through it
  parents spawn the children branches/nodes whatever, lets say branches when we are thinking of
  the git tree, because forall A that are candidates there exists some branch in the git tree.

  So we can just step throguh from the perspective of the branches in the git tree, and an edge is
  a patch, and that patch, in this special case, is using the same runtime as tha rtifact - which
  is not necessary, just convenient for our simplistic prototype which isn't really simple but
  relatively simple to how complicated this gets with multiple rulers in the same tree later, such
  taht each edge is a patch and the ruler that applies that patch is the same as the ruler/runtime
  that is produced by the artifact that branches into those child branches.

  So we can look at it like a comlete git tree, and then step through, maybe displaying the time
  or phases as the nodes/branches being dimmed until we get to that step in the sequence of
  spawned items, and a highlight around whatever is the current Parent<Ruler>, where "current"
  here means the one that is in charge of the epoch of the History that we are using as the
  central measure of time, if not the most granular measure of time or rather of state, because
  History is append-only and blockchain-shaped, so it is the most reliable indicator of the state
  of the graph at any given block height. there are a lot of invariants built into the History
  mutation, and the history mutation is itself append-only and contains a digest, I'm fairly sure.

  So anyway, we highlight the Parent<Ruler>, and then we can step through the graph, where the
  dimmed nodes become non-dimmed when they become real in the sequence. Maybe we add some
  animations or something.

  Then when we step through the graph, we can basically see what happens. But we can also do
  things like include the scores or the fitness or whatever as color-coded nodes along a scale
  like from red to green along a gradient. We can add a bunch of other stuff later to help us
  inspect, some of which we have already kind of worked out in ploke-tree-browser, but that's the
  general shape of the graph at least. We can also add other "Views" of the graph such that the
  same underlying object is being visually represented, conceptually, but maybe we are seeing just
  the sequence of rulers, or maybe something else, idk. theres a lot of data here to work with.

  The main like, low-level detail to keep in mind that I really want to make sure is followed so
  this stays coherent during the whole graph layout and rendering and wahtnot, is that we are not
  going to allow clones of things in the graph. The graph is immutable. if it changes, it is only
  because we have loaded a new one, which we will want to do if we are tracking a live run, which
  wil be often but not like every frame. Runs are pretty slow. Even at the higher granularity most
  of the time we are just waiting for LLM requests over the network, either during child self-
  evalutation or during the follow-up LLM-adjudicated analysis. That is on the order of seconds,
  tens of seconds, or even a couple hundred (usually the longest are around 219 seconds when
  healthy for the runs we've been doing lately, but it could take longer for slower models used
  via OpenRouter). So we can just essentially refresh everything whenever we need to refresh the
  graph. But so the borrow checker here is going to constrain us, and we want that. Because that
  means we can't have like, dead or misattributed links, which would be very easy to do with this
  much data floating around where we might aend up with situations like a runtime revisiting a
  previous artifact to produce a new set of children. I think we already have some confusion
  around this in the graph view, in fact. At least the one shown in the UI.

  So what I mean is we don't want to fight the borrow checker here. We want to let it keep us
  honest. So we aren't running to `.clone()` everything, or use IDs to index stuff which is
  basically a more fallible pointer, or whatever. We want to just take everything by reference,
  and then either use that to derive some other information like the changes over time of, e.g.
  the score of nodes in an ancestry, or a lineage, or the second order changes in time, or things
  like that. Or we could be adding new data from elsewhere that can be cloned when eneded, maybe,
  but the graph can be immutable and the views cana use references, and everything else can use
  references later as well like tables or typed fact views. that also helps us stay
  performant, which is nice, but again, the main idea is to keep us constrained to always
  referering to the same set of underlying objects in the graph here.

  That was a lot of information, but it all kind of comes to a sort of apex in the UI
  representation, and the key is that we want to present a straightfuorward, pleasing UI to start,
  and then make it progressively discoverable by the user, and allow us to drill all the way down
  to a deep level for debugging, but also just allow us or a user to view and appreciate what is
  happening in the graph at a glance, in terms of the most striking points, which is hopefully the
  way that the tree view on its own demonstrates an increase of some metric or oracle score or
  pass rate on multi-swe-bench or something of the runtime harnesses for our ploke-tui agent over
  time, demonstrating that the multi-generation tree of agents framework is effective at its core
  promise, which is to provide improvement over time. All tehe safety stuff is important, for
  sure, but not really representable in a graph like this, and that can be unpacked elsewhere. The
  thing that should just kind of hit you over teh head when you see the graph is that "oh shit,
  this is not vibes, this is real", and then the more technical you are, the more you can drill
  down to a deeper and deper level until you can look at every tool call and every agent turn and
  how many tokens they cost and what they were used to decide and what the LLM-adjuddicated evals
  later said about them, and what the probability distribution (later) is for this set of scores
  on the multi-swe-bench, and so on.
