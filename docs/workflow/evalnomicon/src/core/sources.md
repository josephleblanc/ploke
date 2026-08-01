# Sources and Inspiration

## Key Concepts and Definitions:

#### state
> The set of states E consists of functions u : Loc -> N from locations to numbers. Thus
> u(X) is the value, or contents, of location X in state u.
Winskel, 2.2

Notes:
- `Loc` here is like the set of locations where a given value is stored.

#### commands

##### The execution of commands

> The role of expressions is to evaluate to values in a particular state. The role of a
> program, and so commands, is to execute to change the state. 
Winskel, 2.4

##### (command) configuration

> A pair (c, a) represents the (command) configuration from which it remains
> to execute command c from state a. 
> We shall define a relation
> (c, a) -> a'
> which means the (full) execution of command c in state a terminates in final state a'. 

#### operational semantics
> Because it can be implemented fairly directly
> the rules specify the meaning, or semantics, of arithmetic expressions in an operational
> way, and the rules are said to give an operational semantics of such expressions. 
> The style of semantics we have
> chosen is one which is becoming prevalent however. It is one which is often called
> structural operational semantics because of the syntax-directed way in which the rules
> are presented. It is also called natural semantics because of the way derivations resemble
> proofs in natural deduction-a method of constructing formal proofs. 

## Sources

### Winskel, The Formal Semantics of Programming Languages

> 2.2 The evaluation of arithmetic expressions
> Most probably, the reader has an intuitive model with which to understand the behaviours of programs written in IMP. Underlying most models is an idea of state
> determined by what contents are in the locations. With respect to a state, an arithmetic
> expression evaluates to an integer and a boolean expression evaluates to a truth value.
> The resulting values can influence the execution of commands which will lead to changes
> in state. Our formal description of the behaviour of IMP will follow this line. First we
> define states and then the evaluation of integer and boolean expressions, and finally the
> execution of commands.
> The set of states E consists of functions u : Loc -> N from locations to numbers. Thus
> u(X) is the value, or contents, of location X in state u.
> Consider the evaluation of an arithmetic expression a in a state u. We can represent
> the situation of expression a waiting to be evaluated in state u by the pair (a, u). We
> shall define an evaluation relation between such pairs and numbers
> ```
> (a, u) -> n 
> ```
> meaning: expression a in state a evaluates to n. Call pairs (a, a), where a is an arithmetic
> expression and a is a state, arithmetic-expression configurations.
> Consider how we might explain to someone how to evaluate an arithmetic expression
> (ao + al). We might say something along the lines of:
> 1. Evaluate ao to get a number no as result and
> 2. Evaluate al to get a number nl as result.
> 3. Then add no and nl to get n, say, as the result of evaluating ao + al.
> Although informal we can see that this specifies how to evaluate a sum in terms of how
> to evaluate its summands; the specification is syntax-directed. The formal specification of
> the evaluation relation is given by rules which follow intuitive and informal descriptions
> like this rather closely.
> We specify the evaluation relation in a syntax-directed way, by the following rules:
> Evaluation of numbers:
> (n, a) --> n
> Thus any number is already evaluated with itself as value.
> Evaluation of locations:
> (X, a) --> a(X)
> Thus a location evaluates to its contents in a state.
> Evaluation of sums:
> (ao, a) --> no (aI, a) ---> nl
> (ao + aI, a) ---> n
> where n is the sum of no and nl. 


> 2.4 The execution of commands
> The role of expressions is to evaluate to values in a particular state. The role of a
> program, and so commands, is to execute to change the state. When we execute an
> IMP program we shall assume that initially the state is such that all locations are set to
> zero. So the initial state 0'0 has the property that ao(X) = 0 for all locations X. As we
> all know the execution may terminate in a final state, or may diverge and never yield a
> final state. A pair (c, a) represents the (command) configuration from which it remains
> to execute command c from state a. We shall define a relation
> (c, a) -t a'
> which means the (full) execution of command c in state a terminates in final state a'.
> For example,
> (X := 5, a) -t a'
> where a' is the state a updated to have 5 in location X. We shall use this notation:
> Notation: Let a be a state. Let mEN. Let X E Loc. We write a[mj Xl for the state
> obtained from a by replacing its contents in X by m, i.e. define
> Now we can instead write
> a[mjX](Y) = {~Y) if Y = X,
> if Y =1= X.
> (X:= 5,0') -t a[5jX].
> The execution relation for arbitrary commands and states is given by the following rules
