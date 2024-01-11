use crate::constant::*;
use crate::utils::{merge, PathExt};
use anyhow::Ok;
use rrdt_lib::{
    CompressedParams, Connection, ConnectionBuildResult, ConnectionBuilder, TransportParams,
};
use std::path::Path;
use tokio::fs::File;
use tokio::io::BufReader;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::ToSocketAddrs;
use tokio::task::JoinHandle;

async fn recv_random(mut conn: Connection, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let path = path.as_ref();

    let mut handles = vec![];
    while let Some(mut stream) = conn.accept().await {
        let id = stream.id();

        // 每个stream将接收到的数据先写入临时文件
        let stream_path = path.variant(id)?;

        let handle: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
            let file = File::create(stream_path).await?;

            let mut writer = BufReader::new(file);
            let mut buf = [0u8; 8 * K];

            // let mut total = 0;
            println!("receiving {:?}", stream.id());
            loop {
                let n = stream.recv(&mut buf).await?;

                if n == 0 {
                    break;
                }

                // total += n;
                // eprintln!("{:?}: {:?}", stream.id(), total);

                writer.write(&buf[..n]).await?;
            }

            writer.flush().await?;
            eprintln!("{:?} received", stream.id());

            Ok(())
        });

        handles.push(handle);
    }

    let stream_count = handles.len();
    for handle in handles {
        let _ = handle.await??;
    }

    conn.close().await;

    // 将临时文件合并为最终文件
    merge(path, stream_count).await?;

    Ok(())
}

async fn recv_compressed(
    CompressedParams { byte, mut size }: CompressedParams,
    path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let path = path.as_ref();

    let file: File = File::create(path).await?;
    let mut writer = BufWriter::new(file);

    const BUF_SIZE: usize = 8 * K;
    let buf = [byte; BUF_SIZE];

    while size > 0 {
        let n = std::cmp::min(size as usize, BUF_SIZE);
        writer.write(&buf[..n]).await?;
        size -= n as u64;
    }

    writer.flush().await?;

    Ok(())
}

pub async fn recv(
    local_addr: impl ToSocketAddrs,
    remote_addr: impl ToSocketAddrs,
    path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    println!("receiving");

    let path = path.as_ref();

    let params = TransportParams::default();
    let build = ConnectionBuilder::connect(local_addr, remote_addr)
        .await?
        .with_params(params)
        .build()
        .await?;

    match build {
        ConnectionBuildResult::Connection(conn) => {
            recv_random(conn, path).await?;
            Ok(())
        }
        ConnectionBuildResult::Compressed(params) => {
            recv_compressed(params, path).await?;
            Ok(())
        }
    }
}
