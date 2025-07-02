# ADR-019: File-level Hash Tracking for Change Detection

## Status
PROPOSED

## Context
Our parser currently recognizes node-level changes via granular tracking hashes, but lacks an efficient way to detect file-level modifications at scale. Without file-level hash tracking:
- Incremental processing must rely on expensive content comparisons
- There's no quick way to identify unchanged files
- Caching opportunities are missed for unchanged files

We need a fast, cryptographic method to detect file changes without re-parsing entire codebases.

## Decision
Implement Blake3 hashing for each file during parsing:
1. Add `file_hash: [u8; 32]` field to `ParsedCodeGraph` struct
2. Compute hash during file reading phase in `analyze_file_phase2`
3. Store hash in database for future change detection
4. Use Blake3 algorithm for its cryptographic security and speed advantages

The hash will be computed once per file during parsing and stored alongside the parsed graph data.

## Consequences
- Positive:
  - Enables efficient change detection (hash comparison vs re-parsing)
  - Supports incremental processing optimization
  - Reduces parser workload for unchanged files

- Negative:
  - Adds 32 bytes storage overhead per file
  - Requires Blake3 dependency in syn_parser crate

- Neutral:
  - Hashes are computed during existing file read operation
  - Requires no API changes beyond new hash field

## Compliance
[RFC-002-syn-parser-impl.md](/RFC-002-syn-parser-impl.md) Items:  
- Section 4.3: Processing Pipeline improvements  
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items:  
- CR-009: Value-added dependencies  
- CR-012: Optimized resource utilization  
