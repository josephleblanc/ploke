#[cfg(feature = "metal")]
pub mod metal;

#[cfg(not(feature = "metal"))]
mod metal {}
