**Subtask ID: 1**  
* **Description:** Align type representations with CozoDB schema  
* **Estimated Time:** 4 hours  
* **Cfg Flag Required?:** Yes - `feature_ai_task_cozo1`  
  * Explanation: Changing fundamental type representations (e.g., String→Bytes/Float distinctions) could break existing serialization formats (RON/JSON) and test assertions that rely on current type formatting.  
* **Dependencies:** None  
* **Potential Issues:**  
  - Coercion compatibility between Int/Float in type comparisons  
  - Handling UUID/binary conversions for existing string-based identifiers  
  - Vector type alignment (F32 vs F64 detection)  

**Subtask ID: 2**  
* **Description:** Add `Send + Sync` to all public types  
* **Estimated Time:** 2 hours  
* **Cfg Flag Required?:** No  
  * Rationale: Trait additions don't break existing functionality  
* **Dependencies:** None  
* **Potential Issues:**  
  - Potential need for `Arc<Mutex<>>` patterns in future concurrent access  
  - Third-party type dependencies lacking Send/Sync  

**Subtask ID: 3**  
* **Description:** Streamline AST→CozoDB data flow  
* **Estimated Time:** 6 hours  
* **Cfg Flag Required?:** Yes - `feature_ai_task_cozo3`  
  * Explanation: Removing CodeGraph would break:  
    - All existing serialization tests (RON/JSON)  
    - External integrations using CodeGraph inspection  
    - Current visualization tools (if any)  
* **Dependencies:** 1 (type alignment), 2 (thread safety)  
* **Potential Issues:**  
  - Loss of cross-reference context during streaming  
  - Transaction management for partial parses  
  - Handling nested type relationships without graph context  

**Subtask ID: 4**  
* **Description:** Implement incremental visitor pattern  
* **Estimated Time:** 3 hours  
* **Cfg Flag Required?:** Yes - `feature_ai_task_cozo4`  
  * Explanation: Modifies core visitor logic that current CodeGraph construction relies on  
* **Dependencies:** 3  
* **Potential Issues:**  
  - Order-dependent processing of AST nodes  
  - Early-exit conditions breaking full-traversal expectations  
  - Macro expansion sequencing challenges  

**Execution Order Recommendation:**  
1. Subtask 1 (Type alignment foundation)  
2. Subtask 2 (Non-breaking safety improvements)  
3. Subtask 3 (Core graph removal)  
4. Subtask 4 (Stream optimization)  

Key Risk Mitigation:  
- Maintain both CodeGraph and streaming paths under feature flags during transition  
- Add type conversion shims for backward compatibility  
- Preserve existing test suite under default features
