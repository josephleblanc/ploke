
-----

**NOTE: This is a foundational design document currently under review**
This file is speculative and actively being edited as the proposed
structure for the project. It will continue to be edited as we work on the
proposed project structure and does not accurately reflect the current state of
the project.

This is a planning document **only*** and will be archived once a design
decision is chosen. The only part of this project that is at MVP status so far
is the `syn_parser`, which is the parser for the project.

The user's response **should not** be changed, but you may ask clarifying
questions about the intended project as needed.

-----

**USER DESCRIPTION:**
I am unsure exactly how my project would fit into the categories you listed -
perhaps "Something else" and perhaps "System-level tool". The project is
entirely written in rust, and is an RAG pipeline that parses rust source code
into a useful internal representation, passes the parsed rust source to a
hybrid vector embedding and heterogenous graph database (cozodb). The user's
prompt and contextual gathered by an interface with the user's IDE (our MVP
will handle interfacing with either neovim or rust-analyzer? Not sure how this
will work) are used to query the database, which will contain the vector
embeddings and graph representation of the user's repo, as well as a graph
represenation of key dependencies that are frequently updated or that the LLM
is unlikely to be capable of providing good input for (e.g. bevy). The data
retrieved from the database is then ranked by either a small LLM with low
latency (hosted locally on ollama or something similar - does candle do this?)
or using traditional algorithms. The modified context is then passed to an LLM
that is locally hosted (only locally hosted for first working prototype, add
option for remote handling later). The LLM then provides a response, which may
go down several channels, and may be directly added to some rust files the user
is working on. The interface is either through a CLI or through a locally run
instance of something (egui?) as a GUI. While the user is working on editing
their files, there is a file watcher which is IDE-aware, receiving data on
things like how long the user's cursor idles in one place, what code is
recently added, how long the user stays inside a given function block, and uses
this data to generate additional context that might be helpful for the LLM. The
program is intended to be capable of running completely offline, and optimized
for consumer hardware, with capabilities for cuda optimization, but
specifically (because I want to optimize this for myself personally first) an
rtx 3060 Ti GPU (8GB VRAM), 98000x3d gpu, 32GB RAM, nvme m.2 ssd pcie 4.0 1TB.
The project should be capable of running concurrently. The program should
follow best practices for rust coding and memory safety. The first viable
prototype does not need to satisfy all of the requirements of the previous
description, but should be capable of, at minimum, using cozo to query a
vector/graph database for context to pass to the LLM for code generation and
function as an RAG. The project's structure must allow for the eventual
implementation of everything described so far, however, despite lacking
features in the first viable prototype.

**REASONING**:
First, the main goal is to parse Rust code into an internal representation,
store it in CozoDB, which handles both vector embeddings and graph data. Then,
use user prompts and IDE context to query this database, rank results using
either a small LLM or traditional algorithms, and generate responses through a
local LLM. The interface could be CLI or GUI, and there's a need for file
watchers that are IDE-aware. They want everything to run offline, optimized for
their specific hardware, and follow Rust best practices.

Deployment-wise, they're targeting local hardware with CUDA support.

The existing syn_parser crate is probably part of the parsing component.
Since they're processing Rust code, syn is commonly used, but maybe they
have custom extensions. They need to determine if this is a core part
used by multiple components. Also, deployment-wise, they're targeting
local hardware with CUDA support, so might need conditionally compiled
features for GPU acceleration.

Error handling is crucial. Their CONVENTIONS.md mentions using Result
with `Box<dyn Error>` at boundaries but custom errors internally. So, a
shared error crate might be necessary for cross-component error types,
while each crate handles its own errors.

cross-crate dependencies need to be carefully managed. For example,
the interface crate might depend on context, which in turn uses parser,
embedding, graph_db, and llm. The ide_watcher might need to interact with
context or graph_db to update the database based on user activity.

In term of error strategy, a common error enum in errors that can wrap
module-specific errors using thiserror would help. Each crate can define
its own error types and implement conversions to the shared error type.

Potential challenges: Managing the flow of data between components,
ensuring low-latency for real-time IDE interactions, and handling large
datasets efficiently. Maybe a pipeline architecture where each step is a
modular component that can be tested and optimized separately.

Testing could involve creating a mock database for integration tests and
using example queries to ensure the RAG pipeline works as expected.
Benchmarking the parsing and embedding processes would be important for
performance.
