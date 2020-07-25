#[cfg(test)]
mod tests {
    use super::*;
    #[ignore]
    #[test]
    fn test_super_large_payload() {
        let _large_payload = std::fs::read_to_string("tests/crashing_payload.txt").expect("Couldn't read crashing payload");
        todo!("Write a test for this");
    }
}