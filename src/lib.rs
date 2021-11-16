use std::{
    fs::{File, ReadDir},
    io,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    path::Path,
};

use skyline::nn;

extern "C" {
    #[link_name = "\u{1}_ZN2nn2fs21MountSaveDataForDebugEPKc"]
    pub fn MountSaveDataForDebug(arg1: *const u8) -> i32;
    #[link_name = "\u{1}_ZN2nn2fs6CommitEPKc"]
    pub fn SaveDataCommit(arg1: *const u8) -> i32;
}

/// Abstraction over the configuration directory created for your plugin for the current user.
///
/// Most ``std::fs`` methods to manipulate files have been reimplemented, with extra checks for safety and journaling reasons.
///
/// It is heavily recommended to **NOT** manipulate \"config:/\" yourself, and instead use the methods implemented on ConfigStorage for safety reasons.
pub struct ConfigStorage(std::path::PathBuf);

/// Abstraction over ``std::fs::File`` meant to prevent you from freeing the ``ConfigStorage`` before closing the file.
///
/// All ``std::fs::File`` methods can be used on this structure.
///
/// Since the debug save data is a journalized mount partition, it is possible that files written using this interface do not reflect the changes until the ConfigStorage is dropped.
///
/// If you wish to see immediate changes in the mount partition, consider using the ``write`` method on ``ConfigStorage`` instead,
pub struct ConfigFile<'a>(File, PhantomData<&'a ()>);

impl Deref for ConfigFile<'_> {
    type Target = std::fs::File;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ConfigFile<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl ConfigStorage {
    pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> io::Result<u64> {
        let full_path_from = &self.0.join(from.as_ref());
        let full_path_to = &self.0.join(to.as_ref());

        match std::fs::copy(full_path_from, full_path_to) {
            Ok(res) => {
                self.flush();
                Ok(res)
            },
            Err(err) => Err(err),
        }
    }

    pub fn create<P: AsRef<Path>>(&self, path: P) -> io::Result<ConfigFile<'_>> {
        let full_path = &self.0.join(path.as_ref());

        match std::fs::File::create(full_path) {
            Ok(file) => Ok(ConfigFile(file, PhantomData)),
            Err(err) => Err(err),
        }
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> io::Result<ConfigFile<'_>> {
        let full_path = &self.0.join(path.as_ref());

        match std::fs::File::open(full_path) {
            Ok(file) => Ok(ConfigFile(file, PhantomData)),
            Err(err) => Err(err),
        }
    }

    pub fn remove_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let full_path = &self.0.join(path.as_ref());

        std::fs::remove_file(full_path)?;
        self.flush();
        Ok(())
    }

    pub fn remove_dir<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let full_path = &self.0.join(path.as_ref());

        std::fs::remove_dir(full_path)?;
        self.flush();
        Ok(())
    }

    pub fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let full_path = &self.0.join(path.as_ref());

        std::fs::remove_dir_all(full_path)?;
        self.flush();
        Ok(())
    }

    pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> io::Result<()> {
        let full_path_from = &self.0.join(from.as_ref());
        let full_path_to = &self.0.join(to.as_ref());

        std::fs::rename(full_path_from, full_path_to)?;
        self.flush();
        Ok(())
    }

    pub fn read<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>> {
        let full_path = &self.0.join(path.as_ref());

        std::fs::read(full_path)
    }

    pub fn read_to_string<P: AsRef<Path>>(&self, path: P) -> io::Result<String> {
        let full_path = &self.0.join(path.as_ref());

        std::fs::read_to_string(full_path)
    }

    pub fn read_dir(&self) -> io::Result<ReadDir> {
        std::fs::read_dir(&self.0)
    }

    pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(&self, path: P, contents: C) -> io::Result<()> {
        let full_path = &self.0.join(path.as_ref());

        std::fs::write(full_path, contents)?;
        self.flush();
        Ok(())
    }

    fn flush(&self) {
        unsafe {
            // This is required to actually write the files to the save data, as it is journalized.
            SaveDataCommit(skyline::c_str("config\0"));
        }
    }
}

impl Drop for ConfigStorage {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Mounts the debug save file on \"config:/\" if it isn't already, and then safely creates the directory for your plugin for the current user.
///
/// It is heavily recommended to **NOT** manipulate \"config:/\" yourself, and instead use the returned ``ConfigStorage`` instance for safety reasons.
pub fn acquire_storage<S>(plugin_name: &S) -> Result<ConfigStorage, io::Error>
where
    S: AsRef<str> + ?Sized,
{
    let mut uid = nn::account::Uid { id: [0; 2] };

    unsafe {
        // It is safe to initialize multiple times.
        nn::account::Initialize();
        nn::account::GetLastOpenedUser(&mut uid);

        // Don't check result, we do not care if it is already mounted
        MountSaveDataForDebug(skyline::c_str("config\0"));
    };

    // Generate path for the current user so each user can have their own configuration
    let path = std::path::PathBuf::from("config:/")
        .join(uid.id[0].to_string())
        .join(uid.id[1].to_string())
        .join(plugin_name.as_ref());

    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }

    Ok(ConfigStorage(path))
}
