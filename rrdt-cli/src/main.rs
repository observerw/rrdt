mod constant;
mod utils;

use crate::utils::PathExt;
use clap::{Parser, ValueEnum};
use constant::*;
use std::path::Path;

#[derive(Parser)]
pub struct Args {
    /// 当前端点的角色
    role: Role,
}

#[derive(ValueEnum, Clone)]
enum Role {
    Client,
    Server,
}

pub async fn recv(
    local_addr: impl tokio::net::ToSocketAddrs,
    remote_addr: impl tokio::net::ToSocketAddrs,
    path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    use crate::utils::merge;
    use anyhow::Ok;
    use rrdt_lib::{ConnectionBuilder, TransportParams};
    use tokio::io::AsyncWriteExt;
    use tokio::task::JoinHandle;
    use tokio::{fs::File, io::BufReader};

    println!("receiving");

    let path = path.as_ref();

    let params = TransportParams::default();
    let mut conn = ConnectionBuilder::connect(local_addr, remote_addr)
        .await?
        .with_params(params)
        .build()
        .await?;

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

pub async fn send(
    local_addr: impl tokio::net::ToSocketAddrs,
    path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    use anyhow::Ok;
    use rrdt_lib::{ConnectionListener, TransportParams};
    use tokio::{
        fs::File,
        io::{self, AsyncReadExt, BufReader},
    };

    println!("sending");

    let path = path.as_ref();
    if !path.exists() || !path.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid file path").into());
    }

    let file = File::open(path).await?;
    let meta = file.metadata().await?;
    let len = meta.len();
    let mut reader = BufReader::new(file);
    let mut buf = [0u8; 8 * K];

    eprintln!("file size = {:?}", len);

    let stream_count =
        len / STREAM_CHUNK_SIZE as u64 + (len % STREAM_CHUNK_SIZE as u64 != 0) as u64;

    let params = TransportParams::default().with_streams(stream_count as u16);
    let listener = ConnectionListener::bind(local_addr)
        .await?
        .with_params(params);
    let mut conn = listener.accept().await?;

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

    Ok(())
}

#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
    let Args { role } = Args::parse();

    match role {
        Role::Client => {
            recv(CLIENT_ADDR, SERVER_ADDR, FILE_PATH).await?;
        }
        Role::Server => {
            send(SERVER_ADDR, FILE_PATH).await?;
        }
    }

    Ok(())
}

// #[actix_rt::main]
// async fn main() -> anyhow::Result<()> {
//     use tokio::task::JoinHandle;
//     use tokio::time::Instant;

//     const CLIENT_ADDR: &str = "127.0.0.1:2333";
//     const SERVER_ADDR: &str = "127.0.0.1:2334";
//     const RECV_PATH: &str = "data/target.bin";
//     const SEND_PATH: &str = "data/source.bin";

//     let start = Instant::now();

//     let server: JoinHandle<anyhow::Result<()>> = actix_rt::spawn(async move {
//         send(SERVER_ADDR, SEND_PATH).await?;
//         Ok(())
//     });

//     let client: JoinHandle<anyhow::Result<()>> = actix_rt::spawn(async move {
//         actix_rt::time::sleep(std::time::Duration::from_millis(1)).await;
//         recv(CLIENT_ADDR, SERVER_ADDR, RECV_PATH).await?;
//         Ok(())
//     });

//     let _ = server.await??;
//     let _ = client.await??;

//     let elapsed = start.elapsed();
//     println!("time = {:?}", elapsed);

//     Ok(())
// }
