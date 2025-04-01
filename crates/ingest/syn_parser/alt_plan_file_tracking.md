# Relational-Focused Scope Tracking Plan

## Core Principles
1. **Parser as ETL Pipeline**:
   - Extract: Parse Rust source files
   - Transform: Build intermediate CodeGraph
   - Load: Store in CozoDB via ploke-db

2. **Minimal Scope Resolution**:
   - Only validate basic visibility rules
   - Defer complex queries to CozoDB
   - Focus on accurate data extraction

3. **Relational Modeling**:
   - Design nodes/relations for easy Cozo mapping
   - Prefer flat structures over deep nesting
   - Include all raw data needed for later queries

## Key Changes to Approach

### Data Structure Changes
1. **CodeGraph**:
   - Add `source_file: String` to all nodes
   - Add `raw_visibility: String` (exact syntax)
   - Include span positions for all items

2. **Relations**:
   - Add `relation_source: String` (file path)
   - Include lexical ordering information
   - Track both syntactic and semantic relations

3. **VisitorState**:
   - Add `current_crate: String`
   - Track raw module paths without resolution
   - Store unresolved imports verbatim

### Parser Workflow Changes
1. **Analysis Phase**:
   - Extract raw syntax tree data
   - Build flat node/relation lists
   - Preserve original source structure

2. **Validation Phase**:
   - Run basic scope checks
   - Verify module containment
   - Flag obvious visibility violations

3. **Export Phase**:
   - Serialize to Cozo-friendly format
   - Include all metadata for later querying
   - Preserve source ordering information

### CozoDB Integration Strategy
1. **Node Tables**:
   - `functions(id, name, visibility, source_file, span_start, span_end)`
   - `modules(id, name, path, source_file)`
   - `types(id, name, kind, source_file)`

2. **Relation Tables**:
   - `contains(source_id, target_id, source_file)`
   - `visibility(source_id, target_id, kind, source_file)` 
   - `imports(source_id, target_path, alias, source_file)`

3. **Query Patterns**:
   ```cozo
   # Scope-aware function lookup
   ?[name, body] := *functions{name, body, source_file, span_start, span_end},
                    *visibility[function_id, module_id, 'direct'],
                    current_module = module_id,
                    source_file = current_file
   ```

## Implementation Roadmap
1. **Phase 1 - Data Extraction**:
   - Enhance nodes with source locations
   - Capture raw visibility syntax
   - Preserve import statements verbatim

2. **Phase 2 - Basic Validation**:
   - Verify module containment
   - Check for obvious visibility violations
   - Validate crate boundaries

3. **Phase 3 - Cozo Export**:
   - Implement relational serialization
   - Add export to ploke-db
   - Include position metadata

## Benefits of This Approach
1. **Simpler Parser**:
   - Focuses on extraction over analysis
   - Avoids complex resolution logic
   - Easier to maintain

2. **More Powerful Queries**:
   - Cozo handles complex scope analysis
   - Queries can combine multiple factors
   - Easy to extend with new query patterns

3. **Better Validation**:
   - Database can be revalidated anytime
   - Cross-crate analysis possible
   - Historical queries supported
