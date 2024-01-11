use rrdt_cli::constant::*;
use rrdt_cli::send;

#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
    send(SERVER_ADDR, FILE_PATH).await?;
    Ok(())
}
