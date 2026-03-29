#[cfg(any(unix, target_os = "redox"))]
#[cfg(test)]
mod tests {}

#[cfg(target_os = "windows")]
#[cfg(test)]
mod tests {}
