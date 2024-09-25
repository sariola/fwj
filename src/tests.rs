#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::error::AppError;
    use crate::processing::{process_io_pairs, update_json_file};
    use serde_json::json;
    use tokio::fs;

    #[tokio::test]
    async fn test_update_json_file() -> Result<(), AppError> {
        let temp_dir = tempfile::tempdir()?;
        let file_path = temp_dir.path().join("test.json");
        let initial_content = json!([
            {"input": "test input", "output": "test output"}
        ]);
        fs::write(&file_path, initial_content.to_string()).await?;

        update_json_file(
            file_path.to_str().unwrap(),
            0,
            "feedback",
            json!("test feedback"),
        )
        .await?;

        let updated_content = fs::read_to_string(&file_path).await?;
        let updated_json: serde_json::Value = serde_json::from_str(&updated_content)?;

        assert_eq!(updated_json[0]["feedback"], "test feedback");
        Ok(())
    }

    #[tokio::test]
    async fn test_process_io_pairs() -> Result<(), AppError> {
        // This is a more complex test and might require mocking external dependencies
        // Here's a basic structure:
        let config = Config {
            // ... populate with test values
        };
        let task_config = TaskConfig {
            // ... populate with test values
        };

        // You might need to set up mock files and directories here

        process_io_pairs(&task_config, &config).await?;

        // Assert on the expected outcomes
        // This might involve reading and checking the contents of output files

        Ok(())
    }
}
