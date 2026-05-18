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
pub trait ExtraTrait {}
pub trait AnotherTrait {}

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

pub struct WhereMultiBound<T>(T)
where
    T: LocalTrait + ExtraTrait;

pub struct WhereMultiPredicate<T, U>(T, U)
where
    T: LocalTrait,
    U: ExtraTrait;

pub struct WhereRepeatedSubject<T>(T)
where
    T: LocalTrait,
    T: ExtraTrait;

pub enum WhereEnum<T>
where
    T: LocalTrait,
{
    Variant(T),
}

pub type WhereAliasMulti<T>
where
    T: LocalTrait + ExtraTrait,
 = T;

pub struct WhereImpl<T>(T);

impl<T> LocalTrait for WhereImpl<T>
where
    T: ExtraTrait + AnotherTrait,
{
}

pub struct WhereComposite<T>(T)
where
    Vec<T>: LocalTrait;

pub struct WhereCompositeMulti<T>(T)
where
    Vec<T>: LocalTrait + ExtraTrait;

pub type WhereProjection<T>
where
    <T as LocalAssocBound>::Output: LocalTrait,
 = T;

pub type WhereProjectionMulti<T>
where
    <T as LocalAssocBound>::Output: LocalTrait + ExtraTrait,
 = T;
