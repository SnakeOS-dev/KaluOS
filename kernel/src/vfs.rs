extern crate alloc;

use alloc::{
    string::String,
    vec::Vec,
};

use spin::Mutex;

use crate::ext2::{
    DirEntry,
    Ext2,
    Ext2Error,
    Inode,
    EXT2_ROOT_INO,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsError {
    NotMounted,
    NotFound,
    Exists,
    NotDir,
    IsDir,
    NotEmpty,
    NoSpace,
    Invalid,
    Io,
}

impl From<Ext2Error> for VfsError {
    fn from(
        error: Ext2Error,
    ) -> Self {
        match error {
            Ext2Error::NotFound =>
                Self::NotFound,

            Ext2Error::Exists =>
                Self::Exists,

            Ext2Error::NotDir =>
                Self::NotDir,

            Ext2Error::IsDir =>
                Self::IsDir,

            Ext2Error::NotEmpty =>
                Self::NotEmpty,

            Ext2Error::NoSpace =>
                Self::NoSpace,

            Ext2Error::Invalid
            | Ext2Error::NameTooLong =>
                Self::Invalid,

            _ =>
                Self::Io,
        }
    }
}

#[derive(Clone)]
pub struct Stat {
    pub inode: u32,
    pub mode: u16,
    pub size: u64,
    pub links: u16,
    pub blocks: u32,
    pub is_dir: bool,
    pub is_file: bool,
}

static ROOT_FS:
    Mutex<Option<Ext2>> =
    Mutex::new(None);

pub fn mount_root()
    -> Result<(), VfsError>
{
    let fs =
        Ext2::mount()?;

    *ROOT_FS.lock() =
        Some(fs);

    Ok(())
}

fn with_fs<R>(
    f: impl FnOnce(
        &mut Ext2,
    ) -> Result<R, VfsError>,
) -> Result<R, VfsError> {
    let mut guard =
        ROOT_FS.lock();

    let fs =
        guard
            .as_mut()
            .ok_or(
                VfsError::NotMounted,
            )?;

    f(fs)
}

fn components(
    path: &str,
) -> impl Iterator<Item = &str> {
    path.split('/')
        .filter(
            |part| {
                !part.is_empty()
                    && *part != "."
            },
        )
}

fn resolve_with_fs(
    fs: &mut Ext2,
    path: &str,
) -> Result<u32, VfsError> {
    if !path.starts_with('/') {
        return Err(
            VfsError::Invalid,
        );
    }

    let mut current =
        EXT2_ROOT_INO;

    for part in components(path) {
        if part == ".." {
            current =
                fs.lookup(
                    current,
                    "..",
                )?;

            continue;
        }

        let inode =
            fs.read_inode(
                current,
            )?;

        if !inode.is_dir() {
            return Err(
                VfsError::NotDir,
            );
        }

        current =
            fs.lookup(
                current,
                part,
            )?;
    }

    Ok(current)
}

fn parent_with_fs(
    fs: &mut Ext2,
    path: &str,
) -> Result<(u32, String), VfsError> {
    if !path.starts_with('/')
        || path == "/"
    {
        return Err(
            VfsError::Invalid,
        );
    }

    let trimmed =
        path.trim_end_matches('/');

    let index =
        trimmed
            .rfind('/')
            .ok_or(
                VfsError::Invalid,
            )?;

    let name =
        &trimmed[index + 1..];

    if name.is_empty()
        || name == "."
        || name == ".."
    {
        return Err(
            VfsError::Invalid,
        );
    }

    let parent_path =
        if index == 0 {
            "/"
        } else {
            &trimmed[..index]
        };

    let parent =
        resolve_with_fs(
            fs,
            parent_path,
        )?;

    Ok((
        parent,
        name.into(),
    ))
}

pub fn resolve(
    path: &str,
) -> Result<u32, VfsError> {
    with_fs(
        |fs| {
            resolve_with_fs(
                fs,
                path,
            )
        },
    )
}

pub fn stat(
    path: &str,
) -> Result<Stat, VfsError> {
    with_fs(
        |fs| {
            let ino =
                resolve_with_fs(
                    fs,
                    path,
                )?;

            let inode =
                fs.read_inode(ino)?;

            Ok(stat_from_inode(
                ino,
                &inode,
            ))
        },
    )
}

pub fn read(
    path: &str,
    offset: u64,
    buffer: &mut [u8],
) -> Result<usize, VfsError> {
    with_fs(
        |fs| {
            let ino =
                resolve_with_fs(
                    fs,
                    path,
                )?;

            Ok(
                fs.read_file(
                    ino,
                    offset,
                    buffer,
                )?,
            )
        },
    )
}

pub fn write(
    path: &str,
    offset: u64,
    data: &[u8],
) -> Result<usize, VfsError> {
    with_fs(
        |fs| {
            let ino =
                resolve_with_fs(
                    fs,
                    path,
                )?;

            Ok(
                fs.write_file(
                    ino,
                    offset,
                    data,
                )?,
            )
        },
    )
}

pub fn create(
    path: &str,
    permissions: u16,
) -> Result<u32, VfsError> {
    with_fs(
        |fs| {
            let (
                parent,
                name,
            ) =
                parent_with_fs(
                    fs,
                    path,
                )?;

            Ok(
                fs.create_file(
                    parent,
                    &name,
                    permissions,
                )?,
            )
        },
    )
}

pub fn mkdir(
    path: &str,
    permissions: u16,
) -> Result<u32, VfsError> {
    with_fs(
        |fs| {
            let (
                parent,
                name,
            ) =
                parent_with_fs(
                    fs,
                    path,
                )?;

            Ok(
                fs.mkdir(
                    parent,
                    &name,
                    permissions,
                )?,
            )
        },
    )
}

pub fn unlink(
    path: &str,
) -> Result<(), VfsError> {
    with_fs(
        |fs| {
            let (
                parent,
                name,
            ) =
                parent_with_fs(
                    fs,
                    path,
                )?;

            fs.unlink(
                parent,
                &name,
            )?;

            Ok(())
        },
    )
}

pub fn truncate(
    path: &str,
) -> Result<(), VfsError> {
    with_fs(
        |fs| {
            let ino =
                resolve_with_fs(
                    fs,
                    path,
                )?;

            fs.truncate_zero(
                ino,
            )?;

            Ok(())
        },
    )
}

pub fn readdir(
    path: &str,
) -> Result<Vec<DirEntry>, VfsError> {
    with_fs(
        |fs| {
            let ino =
                resolve_with_fs(
                    fs,
                    path,
                )?;

            Ok(
                fs.read_dir(ino)?,
            )
        },
    )
}

pub fn sync()
    -> Result<(), VfsError>
{
    with_fs(
        |fs| {
            fs.sync()?;
            Ok(())
        },
    )
}

fn stat_from_inode(
    ino: u32,
    inode: &Inode,
) -> Stat {
    Stat {
        inode: ino,
        mode: inode.mode,
        size: inode.size,
        links:
            inode.links_count,
        blocks:
            inode.blocks,
        is_dir:
            inode.is_dir(),
        is_file:
            inode.is_file(),
    }
}
