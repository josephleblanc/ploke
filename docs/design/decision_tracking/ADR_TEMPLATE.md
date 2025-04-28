# ADR template for AI-assisted coding

### Decision Tracking Methodology
1. **ADR Template** (Architecture Decision Record):
```markdown
# ADR-000: [Short Title]

## Status
PROPOSED | ACCEPTED | DEPRECATED

## Context
[Problem description]

## Decision
[Chosen solution]

## Consequences
- Positive:  
- Negative:  
- Neutral:  

## Compliance
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items:  
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:  
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items:  
```

2. **Version Control Strategy**:
- Design documents reside in `/design` directory
- SemVer format: MAJOR.MINOR.PATCH
  - MAJOR: Breaking architectural changes
  - MINOR: New workflow patterns
  - PATCH: Corrections/typos
- ADRs stored in `/design/adrs` numbered sequentially
  - `/design/adrs/proposed/`
  - `/design/adrs/accepted/`
  - `/design/adrs/deprecated/`
- Git tags correspond to document versions

### AI/Human Interface Patterns

#### Feedback Loop Protocol - Conceptual Framework
The collaboration workflow between developer and AI follows a structured review cycle to maintain code quality while enabling rapid iteration. Key components include:

1. **Proposal Generation**: AI generates suggestions with embedded rationales
2. **Annotated Review**: Developer provides targeted feedback
3. **Revision Tracking**: Managed via cryptographic hashes
4. **Architecture Alignment**: Direct links to ADRs and design docs
