#![allow(dead_code, unused_variables)]

pub struct T;
pub struct U;

pub fn concrete(value: T) {}
pub fn also_concrete(value: T) {}
pub fn takes_vec_of_t(value: Vec<T>) {}
pub fn returns_pair() -> (T, U) {
    (T, U)
}

pub fn generic_shadow<T>(value: T) {}

pub trait LocalTrait {}

pub struct LocallyBound<T: LocalTrait>(T);

pub struct UsesTrait;

impl LocalTrait for UsesTrait {}

pub trait ChildTrait: LocalTrait {}

pub struct Const<const N: usize>;

pub trait IntoArrayLength {
    type ArrayLength;
}

pub type ProjectedArrayLength<const N: usize> = <Const<N> as IntoArrayLength>::ArrayLength;

pub trait LocalAssocBound {
    type Output: LocalTrait;
}

pub struct WhereLocal<T>(T)
where
    T: LocalTrait;

pub struct WhereComposite<T>(T)
where
    Vec<T>: LocalTrait;

pub type WhereProjection<T>
where
    <T as LocalAssocBound>::Output: LocalTrait,
 = T;
