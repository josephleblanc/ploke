#![allow(dead_code)]
pub mod printable;
pub mod schema;
pub mod traits;
pub mod transform;

// -- crate-wide imports --

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
