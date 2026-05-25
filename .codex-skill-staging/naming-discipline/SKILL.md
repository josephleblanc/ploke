---
name: naming-discipline
description: Use when evaluating, writing, or refactoring names in code so identifiers do not carry missing semantic structure. Trigger when names become long, repetitive, role-narrating, or prefix-heavy because modules, traits, or types are too weak.
---

# Naming Discipline

## Overview

Names should not compensate for weak structure.

Use this skill when code starts producing identifiers like:
- `InterventionSynthesisContext`
- `RecordedInterventionSynthesisRun`
- `ContextualizeInterventionSynthesis`

Those are usually signs that module boundaries, type carriers, or layering are doing too little work.

## Core Rule

If a name has to explain:
- the subsystem
- the phase
- the role
- the implementation mechanism
- and the local context

then the code is probably missing a structural carrier.

Prefer:
- context from module boundaries
- meaning from trait/type relationships
- short local names inside semantically tight files

Do not rely on identifier length to preserve architecture.

## Workflow

### 1. Identify the missing carrier

Ask what the name is compensating for:
- missing module boundary
- missing trait or type
- mixed concerns in one file
- flattened process with no explicit phases
- generic helper living too far from its semantic home

State the reduction you are refusing:
- "I will not encode module and process structure into one identifier."

### 2. Shorten by strengthening structure

Before renaming, see if one of these should happen first:
- move code into a narrower module
- split one file into semantic subobjects
- introduce an explicit state or phase type
- group related records under one carrier
- separate proposal, realization, and history layers

Then rename in the new context.

### 3. Prefer short local nouns

Inside a semantically narrow module, prefer names like:
- `Context`
- `Draft`
- `Record`
- `Procedure`
- `Apply`
- `Assemble`

Use longer names only when a real ambiguity remains after the structure is fixed.

### 4. Watch for prefix spam

Repeated prefixes like:
- `InterventionX`
- `Prototype1X`
- `ToolTextX`

usually mean the module is too broad.

If many nearby items share the same prefix, move that meaning into:
- the module path
- the enclosing type
- the trait implementation context

Then drop the repeated prefix from the local names.

### 5. Keep semantic titles scarce

Reserve major names like:
- `Intervention`
- `Configuration`
- `History`
- `Protocol`

for the real semantic carriers.

Do not reuse those words in every nearby helper unless the helper truly is that thing.

## Checks

Before accepting a name, ask:
- Would this still need to be this long if the file/module were correct?
- Is the name describing structure that should be encoded elsewhere?
- Are multiple nearby names repeating the same prefix?
- Is this a real semantic object, or a shard of one?

If the answer points to weak structure, fix the structure first.
