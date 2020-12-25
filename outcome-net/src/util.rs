/// Create a valid tcp address that includes the prefix.
pub(crate) fn tcp_endpoint(s: &str) -> String {
    if s.contains("://") {
        s.to_string()
    } else {
        format!("tcp://{}", s)
    }
}
