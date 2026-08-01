
```mermaid
flowchart TB
    subgraph CARRIERS["Left lane: carriers and state chains"]
        direction TB
        PC["Parent carrier"]
        PU["parent unchecked"]
        PK["parent checked"]
        PR["parent ready"]
        PS["parent selectable"]
        PX["parent retired"]
        PC --> PU --> PK --> PR --> PS --> PX

        CC["Child attempt carrier"]
        C1["C1 aligned parent artifact"]
        C2["C2 child artifact"]
        C3["C3 binary present"]
        C4["C4 runtime acknowledged"]
        C5["C5 terminal observed"]
        CC --> C1 --> C2 --> C3 --> C4 --> C5

        RC["Readiness carriers not one chain"]
        RJ["journal evidence"]
        RB["CompleteBaseline"]
        RP["policy and budget"]
        RS["selection evidence"]
        RC --> RJ
        RC --> RB
        RC --> RP
        RC --> RS
    end

    subgraph TRANS["Middle lane: transitions and readiness changes"]
        direction TB
        PT0["load parent"]
        PT1["check checkout"]
        PT2["genesis startup"]
        PT3["predecessor startup"]
        RT0["record parent started"]
        RT1["establish baseline"]
        RT2["reserve planning budget"]
        PT4["receive child-plan authority"]
        RT3["build selection evidence"]
        CT0["admit child attempt"]
        CT1["materialize artifact"]
        CT2["build child binary"]
        CT3["spawn and acknowledge"]
        CT4["observe terminal result"]
        PT5["seal history and retire"]
        PT0 --> PT1 --> PT2
        PT3 --> RT0
        RT0 --> RT1 --> RT2 --> PT4 --> RT3 --> CT0 --> CT1 --> CT2 --> CT3 --> CT4 --> PT5
    end

    subgraph EXEC["Right lane: execution stages"]
        direction TB
        E0["0 CLI dispatch"]
        E1["1 prelude and coordinates"]
        E2{"2 init identity"}
        E2A["2a gen0 identity init"]
        E3["3 parent identity source"]
        E4["4 parent admission"]
        E5["5 parent-start evidence"]
        E6["6 parent baseline"]
        E7["7 policy and budget"]
        E8["8 child-plan resolution"]
        E9["9 child schedule shaping"]
        E10["10 selection strategy"]
        E11{"11 child execution"}
        E11A["11a rejected-only"]
        E11B["11b adaptive fanout"]
        E11C["11c normal fanout"]
        E12["12 report projection"]
        E13["13 successor decision and handoff"]
        E14["14 final report"]
        ER1(["return"])
        ER2(["return"])

        E0 --> E1 --> E2
        E2 -- yes --> E2A --> ER1
        E2 -- no --> E3 --> E4 --> E5 --> E6 --> E7 --> E8 --> E9 --> E10 --> E11
        E11 -- rejected --> E11A --> E12
        E11 -- adaptive --> E11B --> E12
        E11 -- normal --> E11C --> E12
        E12 --> E13 --> E14 --> ER2
    end

    PC ~~~ PT0 ~~~ E0

    E3 -.-> PT0
    E4 -.-> PT1
    E4 -.-> PT2
    E4 -.-> PT3
    E5 -.-> RT0
    E6 -.-> RT1
    E7 -.-> RT2
    E8 -.-> PT4
    E10 -.-> RT3
    E11A -.-> RT3
    E11B -.-> CT0
    E11C -.-> CT0
    E13 -.-> RT3
    E13 -.-> PT5

    PT0 -.-> PU
    PT1 -.-> PK
    PT2 -.-> PR
    PT3 -.-> PR
    PT4 -.-> PS
    PT5 -.-> PX

    RT0 -.-> RJ
    RT1 -.-> RB
    RT2 -.-> RP
    RT3 -.-> RS

    CT0 -.-> C1
    CT1 -.-> C2
    CT2 -.-> C3
    CT3 -.-> C4
    CT4 -.-> C5
```


Annotations:

- The **best-modeled child attempt state** is inside `run_planned_child`: `ChildFiles -> C1 -> C2 -> C3 -> C4 -> C5`.
- The C1-C4 stages are concrete aliases of the same typestate carrier, `Prototype<Running, ArtifactWorld, ChildState, AckState>`. C5 wraps a `C4` plus terminal `ObservedChild` evidence.
- The **best-modeled parent admission state** is `Parent<Unchecked> -> Parent<Checked> -> Parent<Ready>` plus `Startup<Genesis | Predecessor>`.
- The **least-modeled parent-turn state** is the outer orchestration: turn coordinates, evidence scope, baseline readiness, child scheduling, and final report projection remain loose locals.
- The **bootstrap branch** is separate: `--init-parent-identity` commits identity and returns before parent startup, child planning, C1-C5, selection, or successor handoff.
- The **successor boundary** has important pieces (`SelectionSealMaterial`, `Prototype1ContinuationDecision`, `Parent<Selectable> -> Parent<Retired>`), but the caller still handles allowed/stopped continuation as ordinary branch logic.
