#[cfg(any(unix, target_os = "redox"))]
#[cfg(test)]
mod tests {
    fn included_cfg_test_owner() {}
}

#[cfg(target_os = "windows")]
#[cfg(test)]
mod tests {}
