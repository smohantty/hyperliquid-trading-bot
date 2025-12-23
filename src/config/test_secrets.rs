#[cfg(test)]
mod tests {
    use crate::config::read_env_or_file;
    use std::env;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_env_or_file() {
        // Case 1: Direct Env Var
        env::set_var("TEST_SECRET", "direct_value");
        assert_eq!(read_env_or_file("TEST_SECRET").unwrap(), "direct_value");
        env::remove_var("TEST_SECRET");

        // Case 2: File Env Var
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "file_value").unwrap();
        let path = file.path().to_str().unwrap();

        env::set_var("TEST_SECRET_FILE", path);
        assert_eq!(read_env_or_file("TEST_SECRET").unwrap(), "file_value");
        env::remove_var("TEST_SECRET_FILE");

        // Case 3: Missing both
        assert!(read_env_or_file("TEST_SECRET").is_err());
    }
}
