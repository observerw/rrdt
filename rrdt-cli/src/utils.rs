use futures::future::join_all;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use tokio::{
    fs::{self, File},
    io::{self, AsyncReadExt, AsyncSeekExt, BufReader, BufWriter},
};

async fn merge_neighbour(path: &Path, count: usize) -> io::Result<()> {
    let target_path = path.variant(count)?;
    let source_path = path.variant(count + 1)?;

    if !target_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "target file not found",
        ));
    }

    if source_path.exists() {
        let target = File::options().append(true).open(target_path).await?;
        let mut writer = BufWriter::new(target);

        let source = File::open(source_path).await?;
        let mut reader = BufReader::new(source);

        io::copy(&mut reader, &mut writer).await?;
    }

    Ok(())
}

pub async fn merge(path: &Path, mut total: usize) -> io::Result<()> {
    while total > 1 {
        let results = join_all(
            (0..total)
                .step_by(2)
                .map(|count| merge_neighbour(path, count)),
        )
        .await;
        let _ = results.into_iter().collect::<io::Result<Vec<_>>>()?;

        let results = join_all((1..total).step_by(2).map(|count| async move {
            let path = path.variant(count)?;
            fs::remove_file(path).await?;
            Ok(())
        }))
        .await;
        let _ = results.into_iter().collect::<io::Result<Vec<_>>>()?;

        for count in (0..total).step_by(2) {
            let from = path.variant(count)?;
            let to = path.variant(count / 2)?;
            fs::rename(from, to).await?;
        }

        total = (total + 1) / 2;
    }

    let from = path.variant(0)?;
    fs::rename(from, path).await?;

    Ok(())
}

pub trait PathExt {
    fn split_file_name(&self) -> io::Result<(&Path, OsString)>;

    fn variant(&self, variant: impl ToString) -> io::Result<PathBuf>;
}

impl PathExt for Path {
    fn split_file_name(&self) -> io::Result<(&Path, OsString)> {
        let file_name = self
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid file name"))?;

        let file_path = self
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid file path"))?;

        Ok((file_path, file_name.to_os_string()))
    }

    fn variant(&self, variant: impl ToString) -> io::Result<PathBuf> {
        let (file_path, mut file_name) = self.split_file_name()?;
        file_name.push(format!(".{}", variant.to_string()));
        Ok(file_path.join(file_name))
    }
}

/// 检测前64个字节是否相同，如果相同则几乎可以肯定该文件是 `same_char`的，返回该字符
pub async fn try_compress(reader: &mut tokio::io::BufReader<File>) -> io::Result<Option<u8>> {
    let mut buf = [0u8; 64];
    let n = reader.read(&mut buf).await?;
    reader.rewind().await?;

    if n == 0 {
        return Ok(None);
    }

    let is_same = buf.iter().fold(0, |a, b| a ^ b) == 0;
    if is_same {
        Ok(Some(buf[0]))
    } else {
        Ok(None)
    }
}
