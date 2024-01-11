pub mod constant;
mod transport;
pub mod utils;

pub use transport::recv::recv;
pub use transport::send::send;

mod test {
    #[actix_rt::test]
    async fn main() -> anyhow::Result<()> {
        use crate::{recv, send};
        use tokio::task::JoinHandle;
        use tokio::time::Instant;

        const CLIENT_ADDR: &str = "127.0.0.1:2333";
        const SERVER_ADDR: &str = "127.0.0.1:2334";
        const SEND_PATH: &str = "data/source_same.bin";
        const RECV_PATH: &str = "data/target_same.bin";

        let start = Instant::now();

        let server: JoinHandle<anyhow::Result<()>> = actix_rt::spawn(async move {
            send(SERVER_ADDR, SEND_PATH).await?;
            Ok(())
        });

        let client: JoinHandle<anyhow::Result<()>> = actix_rt::spawn(async move {
            actix_rt::time::sleep(std::time::Duration::from_millis(1)).await;
            recv(CLIENT_ADDR, SERVER_ADDR, RECV_PATH).await?;
            Ok(())
        });

        let _ = server.await??;
        let _ = client.await??;

        let elapsed = start.elapsed();
        println!("time = {:?}", elapsed);

        Ok(())
    }
}
