//! 文件日志：当前文件无日期（如 `lingxi.log`），滚动归档带东八区日期；时间戳为东八区。
//!
//! 风格类似 Java Logback：`app.log` → `app.yyyy-MM-dd.log` / `app.yyyy-MM-dd.N.log`。

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use chrono::{DateTime, FixedOffset, Utc};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::MakeWriter;

/// 上海 UTC+8（无夏令时）。
pub fn shanghai_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("UTC+8")
}

fn shanghai_today() -> String {
    Utc::now()
        .with_timezone(&shanghai_offset())
        .format("%Y-%m-%d")
        .to_string()
}

fn shanghai_date_of(sys: SystemTime) -> String {
    let dt: DateTime<Utc> = sys.into();
    dt.with_timezone(&shanghai_offset())
        .format("%Y-%m-%d")
        .to_string()
}

/// 日志行时间：东八区。
#[derive(Clone, Debug, Default)]
pub struct ShanghaiTimer;

impl FormatTime for ShanghaiTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let sh = Utc::now().with_timezone(&shanghai_offset());
        write!(w, "{}", sh.format("%Y-%m-%dT%H:%M:%S%.3f%z"))
    }
}

/// 当前写 `lingxi.log`；超限或跨日时归档为 `lingxi.yyyy-MM-dd[.N].log`。
#[derive(Clone)]
pub struct DateSizeFileWriter {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    dir: PathBuf,
    prefix: String,
    max_bytes: u64,
    /// 当前活动文件所属东八区日期（用于归档命名）。
    date: String,
    file: Option<File>,
    written: u64,
}

impl DateSizeFileWriter {
    pub fn open(dir: impl AsRef<Path>, prefix: &str, max_bytes: u64) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        let mut inner = Inner {
            dir,
            prefix: prefix.to_string(),
            max_bytes: max_bytes.max(1),
            date: shanghai_today(),
            file: None,
            written: 0,
        };
        inner.open_active()?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    pub fn current_path_hint(&self) -> String {
        self.inner
            .lock()
            .map(|g| g.active_path().display().to_string())
            .unwrap_or_else(|_| "(log locked)".into())
    }
}

impl Inner {
    fn active_path(&self) -> PathBuf {
        self.dir.join(format!("{}.log", self.prefix))
    }

    fn archive_path(&self, date: &str, index: u32) -> PathBuf {
        if index == 0 {
            self.dir
                .join(format!("{}.{}.log", self.prefix, date))
        } else {
            self.dir
                .join(format!("{}.{}.{}.log", self.prefix, date, index))
        }
    }

    fn next_archive_path(&self, date: &str) -> PathBuf {
        let mut index = 0u32;
        loop {
            let p = self.archive_path(date, index);
            if !p.exists() {
                return p;
            }
            index = index.saturating_add(1);
        }
    }

    fn open_active(&mut self) -> io::Result<()> {
        let today = shanghai_today();
        let active = self.active_path();

        if active.exists() {
            let meta = fs::metadata(&active)?;
            let file_date = meta
                .modified()
                .ok()
                .map(shanghai_date_of)
                .unwrap_or_else(|| today.clone());
            let len = meta.len();
            // 跨日或已超限：先归档再开新文件
            if file_date != today || len >= self.max_bytes {
                self.archive_active(&file_date)?;
            }
        }

        self.date = today;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.active_path())?;
        self.written = fs::metadata(self.active_path())
            .map(|m| m.len())
            .unwrap_or(0);
        self.file = Some(file);
        Ok(())
    }

    fn archive_active(&mut self, archive_date: &str) -> io::Result<()> {
        if let Some(mut f) = self.file.take() {
            let _ = f.flush();
        }
        let active = self.active_path();
        if !active.exists() {
            return Ok(());
        }
        let dest = self.next_archive_path(archive_date);
        fs::rename(&active, &dest)?;
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        let archive_date = self.date.clone();
        self.archive_active(&archive_date)?;
        self.date = shanghai_today();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.active_path())?;
        self.written = 0;
        self.file = Some(file);
        Ok(())
    }

    fn ensure_ready(&mut self) -> io::Result<()> {
        let today = shanghai_today();
        if self.file.is_none() {
            self.open_active()?;
            return Ok(());
        }
        // 跨东八区自然日：归档后开新 lingxi.log
        if self.date != today {
            self.rotate()?;
        }
        Ok(())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.ensure_ready()?;
        if self.written > 0 && self.written + buf.len() as u64 > self.max_bytes {
            self.rotate()?;
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "log file not open"))?;
        file.write_all(buf)?;
        self.written += buf.len() as u64;
        Ok(())
    }
}

pub struct LockedWriter {
    inner: Arc<Mutex<Inner>>,
}

impl Write for LockedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut g = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log mutex poisoned"))?;
        g.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut g = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log mutex poisoned"))?;
        if let Some(f) = g.file.as_mut() {
            f.flush()?;
        }
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for DateSizeFileWriter {
    type Writer = LockedWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LockedWriter {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn active_file_has_no_date_and_archives_on_size() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lingxi_log_test_{stamp}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let w = DateSizeFileWriter::open(&dir, "lingxi", 64).unwrap();
        let mut out = w.make_writer();
        out.write_all(b"abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOP")
            .unwrap();
        out.write_all(b"XXXXXXXXXXXXXXXXXXXXXXXXXXXX").unwrap();
        out.flush().unwrap();

        let active = dir.join("lingxi.log");
        let today = shanghai_today();
        let archived = dir.join(format!("lingxi.{today}.log"));
        assert!(active.exists(), "missing active {active:?}");
        assert!(archived.exists(), "missing archive {archived:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}
