#![forbid(unsafe_code)]
#[tokio::main]
async fn main() -> std::process::ExitCode {
    let result = match std::env::var("KARATE_ERASURE_DATABASE_URL") {
        Ok(connection) => karate_erasure::run(&connection).await,
        Err(_) => Err(karate_core::Error::Validation),
    };
    match result {
        Ok((outcome, deleted_rows)) => {
            println!(
                "{}",
                serde_json::json!({"outcome":outcome,"deleted_rows":deleted_rows})
            );
            std::process::ExitCode::SUCCESS
        }
        Err(_) => {
            eprintln!(
                "Erasure batch could not run. Check the documented role, schema and configuration; no completion is claimed."
            );
            std::process::ExitCode::FAILURE
        }
    }
}
