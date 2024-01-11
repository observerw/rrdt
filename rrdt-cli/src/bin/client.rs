use rrdt_cli::constant::*;
use rrdt_cli::recv;

#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
    recv(CLIENT_ADDR, SERVER_ADDR, FILE_PATH).await?;
    Ok(())
}
