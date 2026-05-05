#![allow(dead_code, unused_variables)]

pub struct T;

pub fn concrete(value: T) {}

pub fn generic_shadow<T>(value: T) {}

pub trait LocalTrait {}

pub struct UsesTrait;

impl LocalTrait for UsesTrait {}

pub trait ChildTrait: LocalTrait {}
