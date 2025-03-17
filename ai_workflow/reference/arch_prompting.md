# Prompt Reference
This is a place for examples and templates of prompts:

## Project Scoping
Ask for technology-specific comparisons: 

```
[PROJECT SCOPING REQUEST] 
Compare storage options for code relationships: 
- cozoDB vs Neo4j vs pgvector 
Key factors for our case: 
1. Embedding search speed 
2. Rust-native integration 
3. Low memory overhead 
Include sample schema sketches for chosen option. 
```

## Constraint Validation
```
VERIFY: Can syn's visitor pattern capture: 
- Function call graphs 
- Trait implementation hierarchies 
- Type parameter flows 
Given our need for cross-file relationships, 
suggest parsing strategy. 
```

## Decision Tree
```
  USER: Compare these AST representation strategies for Rust code: 
  1. Raw syntax tree storage 
  2. Custom semantic graph nodes 
  3. Hybrid AST+type system notation 
  
  Consider: 
  - Query efficiency for code completion 
  - Memory footprint on consumer hardware 
  - Integration with 7B-parameter LLMs 
  
  AI RESPONSE: 
  1. **Raw AST Storage** 
  (+) Full fidelity 
  (-) High memory (2.3GB per 100k LOC) 
  
  2. **Semantic Nodes** 
  (+) Faster queries for common patterns 
  (-) Loses rare edge cases 
  
  3. **Hybrid Approach** 
  (Recommended) Store AST with type annotations 
  Balance: 1.7GB/100k LOC + 89% query accuracy 
```

## Risk Assessment
```
  RISK ASSESSMENT PROMPT: 
  "Verify if these components can fit in 24GB VRAM: 
  - 7B model (INT4 quantized) 
  - cozoDB with 500k code entities 
  - Embedding model (384-dim) 
  
  Include fallback strategies for 12GB systems." 
```
