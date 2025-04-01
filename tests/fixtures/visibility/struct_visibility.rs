mod private_mod {
    #[derive(Default)]
    pub struct PublicStructInPrivate {
        field: i8,
    }

    impl PublicStructInPrivate {
        fn private_assoc_func() {}
        pub fn pub_assoc_func() {}
        fn private_method(&self) {}
        pub fn pub_method(&self) {}
    }

    #[derive(Default)]
    struct PrivateStructInPrivate {
        field: i8,
    }

    impl PrivateStructInPrivate {
        fn private_assoc_func() {}
        pub fn pub_assoc_func() {}
        fn private_method(&self) {}
        pub fn pub_method(&self) {}
    }
}
pub mod public_mod {
    #[derive(Default)]
    pub struct PublicStructInPublic {
        field: i8,
    }

    impl PublicStructInPublic {
        fn private_assoc_func() {}
        pub fn pub_assoc_func() {}
        fn private_method(&self) {}
        pub fn pub_method(&self) {}
    }

    #[derive(Default)]
    struct PrivateStructInPublic {
        field: i8,
    }

    impl PrivateStructInPublic {
        fn private_assoc_func() {}
        pub fn pub_assoc_func() {}
        fn private_method(&self) {}
        pub fn pub_method(&self) {}
    }
}
mod first_example {
    // Which of the following are valid here? Why or why not?
    use super::private_mod::PrivateStructInPrivate; // E0603 struct ... is private
    use super::private_mod::PublicStructInPrivate;

    fn some_func(private_struct: PrivateStructInPrivate, public_struct: PublicStructInPrivate) {
        PrivateStructInPrivate::private_assoc_func(); // Invalid, PrivateStructInPrivate out of
                                                      // scope
        PrivateStructInPrivate::pub_assoc_func(); // Invalid, PrivateStructInPrivate out of scope
        private_struct.private_method(); // Invalid, PrivateStructInPrivate out of scope
        private_struct.pub_method(); // Invalid, PrivateStructInPrivate out of scope
        PublicStructInPrivate::private_assoc_func(); // E0433 failed to resolve
        PublicStructInPrivate::pub_assoc_func(); // Correct
        public_struct.private_method(); // E0624 method ... is private
        public_struct.pub_method(); // Correct
    }
}

pub mod second_example {
    use super::public_mod::PrivateStructInPublic; // E0603 struct ... is private
    use super::public_mod::PublicStructInPublic;
    fn some_func(private_struct: PrivateStructInPublic, public_struct: PublicStructInPublic) {
        PrivateStructInPublic::private_assoc_func(); // Error because PrivateStructInPublic out of
                                                     // scope
        PrivateStructInPublic::pub_assoc_func(); // Error because PrivateStructInPublic out of scope
        private_struct.private_method(); // Invalid, PrivateStructInPublic out of scope
        private_struct.pub_method(); // Invalid, PrivateStructInPublic out of scope
        PublicStructInPublic::private_assoc_func(); // E0624 method ... is private
        PublicStructInPublic::pub_assoc_func(); // Correct
        public_struct.private_method(); // E0624 method ... is private
        public_struct.pub_method(); // Correct
    }
}
