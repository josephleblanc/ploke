//! Helper macros for mock_serde
//!
//! This module provides macros for implementing Deserializer.
//! Note: The main forward_to_deserialize_any macro is re-exported from
/// mock_serde_core. Additional helper macros can be defined here.

#[doc(hidden)]
#[macro_export]
macro_rules! forward_to_deserialize_any_method {
    ($func:ident<$l:tt, $v:ident>($($arg:ident : $ty:ty),*)) => {
        #[inline]
        fn $func<$v>(self, $($arg: $ty,)* visitor: $v) -> Result<$v::Value, <Self as $crate::de::Deserializer<$l>>::Error>
        where
            $v: $crate::de::Visitor<$l>,
        {
            $(let _ = $arg;)*
            self.deserialize_any(visitor)
        }
    };
}

#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! forward_to_deserialize_any_helper {
    (bool<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_bool<$l, $v>()}
    };
    (i8<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_i8<$l, $v>()}
    };
    (i32<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_i32<$l, $v>()}
    };
    (i64<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_i64<$l, $v>()}
    };
    (u8<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_u8<$l, $v>()}
    };
    (u32<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_u32<$l, $v>()}
    };
    (u64<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_u64<$l, $v>()}
    };
    (f32<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_f32<$l, $v>()}
    };
    (f64<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_f64<$l, $v>()}
    };
    (string<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_string<$l, $v>()}
    };
    (option<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_option<$l, $v>()}
    };
    (unit<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_unit<$l, $v>()}
    };
    (seq<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_seq<$l, $v>()}
    };
    (map<$l:tt, $v:ident>) => {
        forward_to_deserialize_any_method!{deserialize_map<$l, $v>()}
    };
}
