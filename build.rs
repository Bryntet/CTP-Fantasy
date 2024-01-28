
use std::env;
fn main() {
    dotenvy::dotenv().ok();

    // List of required environment variables
    let required_vars = ["DATABASE_URL", "FLUTTER_PATH"];

    for &var in &required_vars {
        match env::var(var) {
            Ok(value) => {
                println!("{} is set to {}", var, value);
                // You can use the value here as needed
            },
            Err(_) => {
                println!("cargo:warning=Required environment variable {} is not set.", var);
            }
        }
    }

    // Rest of your build script logic
}