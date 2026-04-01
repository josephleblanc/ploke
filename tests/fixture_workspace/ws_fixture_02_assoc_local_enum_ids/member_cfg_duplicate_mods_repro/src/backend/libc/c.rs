// Mirrored shape from rustix: two cfg-gated modules with the same name.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod readwrite_pv64v2 {
    pub(super) fn preadv64v2() -> i32 {
        0
    }
}

#[cfg(any(
    target_os = "android",
    all(target_os = "linux", not(target_env = "gnu")),
))]
mod readwrite_pv64v2 {
    pub(super) fn preadv64v2() -> i32 {
        1
    }
}
