pub mod error;
pub mod types;

pub use error::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = FaberError::Sandbox("test error".to_string());
        assert_eq!(error.to_string(), "Sandbox error: test error");
    }

    #[test]
    fn test_result_handling() {
        let result: Result<()> = Ok(());
        assert!(result.is_ok());

        let error_result: Result<()> = Err(FaberError::Sandbox("test".to_string()));
        assert!(error_result.is_err());
    }
}
