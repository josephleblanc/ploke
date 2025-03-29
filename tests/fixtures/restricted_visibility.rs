mod outer {
    mod middle {
        pub(in crate::outer) fn restricted_fn() {}
        
        pub(in crate::outer) struct RestrictedStruct;
    }

    pub fn access_restricted() {
        middle::restricted_fn();
        let _ = middle::RestrictedStruct;
    }
}

mod unrelated {
    pub fn cannot_access() {
        // These would cause compiler errors:
        // crate::outer::middle::restricted_fn();
        // let _ = crate::outer::middle::RestrictedStruct;
    }
}
