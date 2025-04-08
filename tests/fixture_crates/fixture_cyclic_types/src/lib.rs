#![allow(dead_code)]

// 1. Struct with direct self-reference via Box
pub struct LinkedListNode {
    data: i32,
    next: Option<Box<LinkedListNode>>,
}

// 2. Mutually recursive structs via Box
pub struct Tree {
    root: Option<Box<TreeNode>>,
}

pub struct TreeNode {
    data: String,
    children: Vec<Tree>, // Refers back to Tree
}

// 3. Enum with self-reference via Box
pub enum Expr {
    Number(i32),
    Add(Box<Expr>, Box<Expr>),
    Subtract(Box<Expr>, Box<Expr>),
}

// 4. Type alias involving recursion (less direct cycle) - Causes E0391
// pub type NestedList = Option<Box<(i32, NestedList)>>;
//
// pub fn process_nested(list: NestedList) -> i32 {
//     match list {
//         Some(boxed_tuple) => boxed_tuple.0 + process_nested(boxed_tuple.1),
//         None => 0,
//     }
// }
// Using a struct instead to represent the same recursive structure for parsing tests:
pub struct NestedListStruct {
    data: i32,
    next: Option<Box<NestedListStruct>>,
}


// 5. Cyclic dependency through traits (harder to represent directly,
//    but type references might form cycles in the TypeGraph)
pub trait Ping {
    type Ponger: Pong<Pinger = Self>;
    fn ping(&self, ponger: &Self::Ponger);
}

pub trait Pong {
    type Pinger: Ping<Ponger = Self>;
    fn pong(&self, pinger: &Self::Pinger);
}

pub struct A;
pub struct B;

impl Ping for A {
    type Ponger = B; // A refers to B
    fn ping(&self, ponger: &Self::Ponger) { ponger.pong(self); }
}

impl Pong for B {
    type Pinger = A; // B refers to A
    fn pong(&self, pinger: &Self::Pinger) { pinger.ping(self); }
}
