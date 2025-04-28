# Error Documentation Template

## File Purpose
This template ensures consistent documentation of:
- Common errors encountered during AI-assisted development
- Root cause analysis
- Prevention strategies
- Links to related insights

## Required Sections
1. **Error Header**
   - Code and brief description
   - Severity level (Critical/Warning/Info)

2. **Context**  
   - Development phase when occurred
   - Specific operation being attempted
   - Relevant code sections

3. **Root Causes**
   - Technical factors
   - Workflow factors
   - System limitations

4. **Prevention**
   - Immediate fixes
   - Long-term solutions
   - Tooling improvements

5. **Links**
   - Related errors
   - Relevant insights
   - Any connected ADRs

## Example Structure
```markdown
### Error E0499: Multiple Mutable Borrows  
**Description**: Attempting concurrent mutable access to visitor state  

**Context**:  
- During module item processing  
- While tracking module paths  

**Root Causes**:  
1. Nested mutation requirements  
2. Missing borrow separation  
3. Complex visitor flow  

**Prevention**:  
- Phase separation pattern  
- Interior mutability  
- Ownership visualization  

[See Insights](#potential-insights-from-e0499)  
```

[Back to Common Errors](#common-error-patterns-in-ai-assisted-development)
