// ploke-embed
//
// This crate is mostly a stub for now. It's role in the overall structure is to manage the
// creation of vector embeddings.
//
// The current architecture relies solely on the `cozo` dependency to handle vector embeddings, as
// `cozo` has a feature which allows for automatically creating vector embeddings. However, in the
// long term it will be good to create our own functions to handle this functionality, allowing
// for user-defined back-ends to handle vector embeddings (e.g. ollama)
//
// Actually creating the embeddings is trivial when using the built-in cozo functionality, so this
// crate will not be doing any work for a while. Instead, we will be using the `database` crate to
// handle test queries involving embeddings, as the `database` crate has responsibility for
// creating the pre-formed queries that will be used across the crate.
//
// However, for the MVP we will rely only on `cozo` to avoid the complexity of handling embeddings,
// particularly the complexity of handling embeddings outside rust, for example ollama or an
// external language interface like pytorch.

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
