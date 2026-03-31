// tests on items within an executable scope, like a function body or closure.
mod executable_scope;
// tests on items within a secondary scope, such as within an enum definition
mod secondary_scope;
// tests on consts that are siblings in primary scope, such as consts defined with a direct module
// parent
mod const_underscore;
