use crate::constant::*;
use crate::utils::try_compress;
use anyhow::Ok;
use rrdt_lib::TransportParams;
use rrdt_lib::{CompressedParams, ConnectionListener};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, BufReader};
use tokio::net::ToSocketAddrs;

async fn send_compressed(
    local_addr: impl ToSocketAddrs,
    params: CompressedParams,
) -> anyhow::Result<()> {
    let listener = ConnectionListener::bind(local_addr)
        .await?
        .with_compressed_params(params);

    match listener.accept().await? {
        None => Ok(()),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "invalid data").into()),
    }
}

pub async fn send(local_addr: impl ToSocketAddrs, path: impl AsRef<Path>) -> anyhow::Result<()> {
    println!("sending");

    let path = path.as_ref();
    if !path.exists() || !path.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid file path").into());
    }

    let file = File::open(path).await?;
    let meta = file.metadata().await?;
    let len = meta.len();
    let mut reader = BufReader::new(file);

    eprintln!("file size = {:?}", len);

    let compressed = try_compress(&mut reader).await?;

    if let Some(byte) = compressed {
        let params = CompressedParams { byte, size: len };
        send_compressed(local_addr, params).await
    } else {
        let stream_count =
            len / STREAM_CHUNK_SIZE as u64 + (len % STREAM_CHUNK_SIZE as u64 != 0) as u64;

        let params = TransportParams::default().with_streams(stream_count as u16);
        let listener = ConnectionListener::bind(local_addr)
            .await?
            .with_transport_params(params);

        if let Some(mut conn) = listener.accept().await? {
            let mut buf = [0u8; 8 * K];
            for _ in 0..stream_count {
                let mut stream = conn.open().await;
                let mut total = 0;
                eprintln!("sending {:?}", stream.id());
                loop {
                    let n = reader.read(&mut buf).await?;
                    if n == 0 {
                        break;
                    }

                    stream.send(&buf[..n]).await?;

                    total += n;

                    if total >= STREAM_CHUNK_SIZE {
                        break;
                    }
                }

                stream.wrote();
                eprintln!("{:?} sent", stream.id());
            }

            conn.close().await;
        }

        Ok(())
    }
}
