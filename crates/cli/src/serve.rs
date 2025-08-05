use tracing::info;

use crate::types::ServeOptions;

pub fn serve(options: ServeOptions) -> Result<(), Box<dyn std::error::Error>> {
    info!("Serving...");

    Ok(())
}
