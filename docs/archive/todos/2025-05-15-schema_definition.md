------------------------------------------------------------
----------------- To Implement -----------------------------
------------------------------------------------------------

NOTE: More notes in transform/edges.rs regarding explicit edges

Nodes:
 - [✔] Const
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing
 - [✔] Static
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing
 - [✔] Struct
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing
     - [✔] Field
         - [✔] Define Schema (*NodeSchema)
         - [✔] Add edge (implicit in ParamData owner_id)
         - [✔] Define tranform
             - [✔] Basic testing
         - [ ] Add explicit edges Struct->Field
 - [✔] Enum
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing
     - [✔] Variant
         - [✔] Define Schema (*NodeSchema)
         - [✔] Add edge (implicit in Variant owner_id)
         - [ ] Add explicit edge (SyntacticRelation)
     - [✔] Field (if different from struct field)
         - [✔] Add edge (implicit in Field owner_id)
         - [ ] Add explicit edge
 - [✔] TypeAlias
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing
 - [✔] Union
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing
 - [✔] Function
     - [✔] Define tranform
         - [✔] Basic testing
     - [✔] Define Schema (*NodeSchema)
     - [✔] ParamData
         - [✔] Define tranform
             - [✔] Basic testing
         - [✔] Define Schema (*NodeSchema)
         - [✔] Add edge (implicit in ParamData owner_id)
         - [ ] Add explecit edge Function->ParamData
 - [✔] Impl
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing
     - [✔] Method
         - [✔] Define Schema (*NodeSchema)
         - [✔] Define tranform
             - [✔] Basic testing
             - [ ] Add own basic test
         - [✔] Add edge (implicit in method field: owner_id)
         - [✔] Add edge (implicit in impl field: methods)
         - [ ] Add explicit edge
 - [✔] Macro
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing
 - [✔] Trait
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing
     - [✔] Method (if different from impl method)
         - [✔] Basic testing
         - [✔] Add edge (implicit in method field: owner_id)
         - [✔] Add edge (implicit in trait field: methods)
         - [ ] Add explicit edge
 - [✔] Module (Not split, rather using adjacent nodes for file/inline/decl for now)
     - [✔] Define Schema (DeclModuleNodeSchema)
     - [✔] Define tranform
     - [✔] FileModuleNode (if different from impl method)
         - [✔] Basic testing
         - [✔] Add edge (implicit in method field: owner_id)
         - [✔] Basic testing
 - [ ] Import
     - [✔] Define Schema (*NodeSchema)
     - [✔] Define tranform
         - [✔] Basic testing

Add Schema definitions for Associated Nodes:
 - [✔] MethodNodeSchema
 - [ ] AssociatedConstNode (not tracked yet)
 - [ ] AssociatedStaticNode (not tracked yet)
 - [ ] AssociatedFunctionNode (not tracked yet?)
     - I think we don't track this yet? Check on this.

 - [✔] ParamData
     - [✔] Add function to turn into BTree
 - [✔] VariantNode
     - [✔] Add function to turn into BTree
 - [✔] FieldNode
     - [✔] Add function to turn into BTree
 - [✔] GenericParamNode
     - [✔] GenericTypeNodeSchema
     - [✔] GenericLifetimeNodeSchema
     - [✔] GenericConstNodeSchema
     - [✔] Add function to turn into BTree
 - [✔] Attribute
     - [✔] Add function to turn into BTree

