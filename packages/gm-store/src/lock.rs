use crate::error::{Error,
                   IoContext,
                   Result};
use fs2::FileExt;
use std::{fs::{File,
               OpenOptions},
          path::{Path,
                 PathBuf}};

#[derive(Debug)]
pub struct ProjectLock {
    _file: File,
    path: PathBuf,
}

impl ProjectLock {
    pub(crate) fn acquire(path: &Path) -> Result<ProjectLock> {
        let lock = Self::open(path)?;
        lock._file.try_lock_exclusive().map_err(|_| Error::Locked(path.to_path_buf()))?;
        Ok(lock)
    }

    pub fn acquire_wait(path: &Path) -> Result<ProjectLock> {
        let lock = Self::open(path)?;
        lock._file.lock_exclusive().ctx(format!("locking {}", path.display()))?;
        Ok(lock)
    }

    fn open(path: &Path) -> Result<ProjectLock> {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .ctx(format!("opening lock file {}", path.display()))?;
        Ok(ProjectLock {
            _file: file,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path { &self.path }
}
