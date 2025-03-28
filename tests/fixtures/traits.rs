
pub trait SampleTrait {
    fn sample_method(&self) -> String;
}

pub trait DefaultTrait {
    fn default_method(&self) -> String {
        "Default implementation".to_string()
    }
}

pub trait GenericTrait<T> {
    fn generic_method(&self, param: T) -> T;
}

pub trait assoc_type_trait {
    type AssocType;
    fn method_with_assoc(&self) -> Self::AssocType;
}

trait PrivateTrait {
    fn private_method(&self) -> i32;
}
