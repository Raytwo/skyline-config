use std::{
    fs::ReadDir,
    io,
    path::{Path, PathBuf},
    str::FromStr,
};

use skyline::nn;
use thiserror::Error;

extern "C" {
    #[link_name = "\u{1}_ZN2nn2fs21MountSaveDataForDebugEPKc"]
    pub fn MountSaveDataForDebug(arg1: *const u8) -> i32;
    #[link_name = "\u{1}_ZN2nn2fs6CommitEPKc"]
    pub fn SaveDataCommit(arg1: *const u8) -> i32;
    #[link_name = "\u{1}_ZN2nn7account9GetUserIdEPNS0_3UidERKNS0_10UserHandleE"]
    pub fn get_user_id(arg1: &mut nn::account::Uid, handle: &UserHandle) -> i32;
    #[link_name = "\u{1}_ZN2nn7account22TryOpenPreselectedUserEPNS0_10UserHandleE"]
    pub fn open_preselected_user(handle: &mut UserHandle) -> bool;
    #[link_name = "\u{1}_ZN2nn7account9CloseUserERKNS0_10UserHandleE"]
    pub fn close_user(handle: &UserHandle) -> u32;
}

pub struct UserHandle([u64; 3]);

impl UserHandle {
    pub fn new() -> Self {
        UserHandle([0u64; 3])
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to perform an operation on the file")]
    FileError(#[from] std::io::Error),
    #[error("the requested field could not be found")]
    FieldMissing,
    #[error("failed to call from_str for the desired type")]
    FromStrErr,
}

pub struct StorageHolder<CS: ConfigStorage>(CS);

pub struct SdCardStorage(std::path::PathBuf);

impl SdCardStorage {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self(path.into())
    }
}

impl ConfigStorage for SdCardStorage {
    fn initialize(&self) -> Result<(), ConfigError> {
        // TODO: Check if the SD is mounted or something
        let path = self.storage_path();

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        Ok(())
    }

    fn root_path(&self) -> PathBuf {
        PathBuf::from("sd:/")
    }

    fn storage_path(&self) -> PathBuf {
        self.root_path().join(&self.0)
    }
}

/// Abstraction over the configuration directory created for your plugin for the current user.
///
/// It is heavily recommended to **NOT** manipulate \"config:/\" yourself, and instead use the methods implemented on ConfigStorage for safety reasons.
pub struct DebugSavedataStorage(std::path::PathBuf);

impl DebugSavedataStorage {
    pub fn new<P: AsRef<Path>>(plugin_name: P) -> Self {
        let mut uid = nn::account::Uid { id: [0; 2] };
        let mut handle = UserHandle([0u64; 3]);

        unsafe {
            // It is safe to initialize multiple times.
            nn::account::Initialize();

            // This provides a UserHandle and sets the User in a Open state to be used.
            if !open_preselected_user(&mut handle) {
                panic!("OpenPreselectedUser returned false");
            }

            // Obtain the UID for this user
            get_user_id(&mut uid, &handle);
            // This closes the UserHandle, making it unusable, and sets the User in a Closed state.
            close_user(&handle);
            // Make sure we can't use Handle from here
            drop(handle);
        }

        // Generate path for the current user so each user can have their own configuration
        let path = PathBuf::from(uid.id[0].to_string()).join(uid.id[1].to_string()).join(plugin_name);

        Self(path)
    }
}

impl ConfigStorage for DebugSavedataStorage {
    fn initialize(&self) -> Result<(), ConfigError> {
        unsafe {
            // Don't check result, we do not care if it is already mounted
            MountSaveDataForDebug(skyline::c_str("config\0"));
        }

        let path = self.storage_path();

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        Ok(())
    }

    fn root_path(&self) -> PathBuf {
        PathBuf::from("config:/")
    }

    fn storage_path(&self) -> PathBuf {
        self.root_path().join(&self.0)
    }

    fn require_flushing(&self) -> bool {
        true
    }

    fn perform_flush(&self) {
        unsafe {
            // This is required to actually write the files to the save data, as it is journalized.
            SaveDataCommit(skyline::c_str("config\0"));
        }
    }
}

impl Drop for DebugSavedataStorage {
    fn drop(&mut self) {
        self.perform_flush();
    }
}

impl<CS: ConfigStorage> StorageHolder<CS> {
    // /// TODO: Rework this to allow copying the config from one user to the other using the UID to compute paths.
    // fn copy<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> io::Result<u64> {
    //     todo!();

    //     let full_path_from = self.0.join(from);
    //     let full_path_to = self.0.join(to);

    //     std::fs::copy(full_path_from, full_path_to).map(|res| {
    //         self.flush();
    //         res
    //     })
    // }

    pub fn new(storage: CS) -> Self {
        storage.initialize().unwrap();
        Self(storage)
    }

    fn create<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let full_path = self.0.storage_path().join(path);

        std::fs::File::create(full_path).map(|_| ())
    }

    fn remove_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let full_path = self.0.storage_path().join(path);

        std::fs::remove_file(full_path)?;
        self.flush();
        Ok(())
    }

    /// Renames a field or a flag to another name.
    pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(&mut self, from: P, to: Q) -> Result<(), ConfigError> {
        let full_path_from = self.0.storage_path().join(from);
        let full_path_to = self.0.storage_path().join(to);

        std::fs::rename(full_path_from, full_path_to)?;
        self.flush();
        Ok(())
    }

    /// Abstraction of ``std::fs::read_dir`` over the Configuration Storage.
    pub fn read_dir(&self) -> io::Result<ReadDir> {
        std::fs::read_dir(&self.0.storage_path())
    }

    fn read_to_string<P: AsRef<Path>>(&self, path: P) -> io::Result<String> {
        let full_path = self.0.storage_path().join(path);

        std::fs::read_to_string(full_path)
    }

    fn write<P: AsRef<Path>, C: AsRef<[u8]>>(&mut self, path: P, contents: C) -> Result<(), ConfigError> {
        let full_path = self.0.storage_path().join(path);

        std::fs::write(full_path, contents)?;
        self.flush();
        Ok(())
    }

    /// Provides the value of the field if it exists.
    /// Use type inference to specify the type it should deserialize to.
    pub fn get_field<T: FromStr>(&self, path: impl AsRef<Path>) -> Result<T, ConfigError> {
        let test = self.read_to_string(path).map_err(|_| ConfigError::FieldMissing)?;
        T::from_str(test.as_str()).map_err(|_| ConfigError::FromStrErr)
    }

    /// Create a field in the configuration and assigns it the value provided
    pub fn set_field(&mut self, path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<(), ConfigError> {
        Ok(self.write(path, contents)?)
    }

    /// Remove a field from the configuration, along with its value
    pub fn remove_field(&mut self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        Ok(self.remove_file(path)?)
    }

    /// Checks if a flag is enabled in the configuration
    pub fn get_flag<P: AsRef<Path>>(&self, path: P) -> bool {
        let full_path = self.0.storage_path().join(path);
        std::path::Path::exists(&full_path)
    }

    /// If ``flag`` is set to true, enable the flag if it isn't already set.
    /// Otherwise, disable the flag
    pub fn set_flag<P: AsRef<Path>>(&mut self, path: P, flag: bool) -> Result<(), ConfigError> {
        if flag {
            Ok(self.create(path)?)
        } else {
            Ok(self.remove_file(path)?)
        }
    }

    /// Delete every file in the configuration storage.
    /// Be absolutely sure this is what you desire before calling it.
    pub fn clear_storage(&mut self) {
        self.read_dir().unwrap().for_each(|entry| {
            std::fs::remove_file(entry.unwrap().path()).unwrap();
        });

        self.flush();
    }

    pub fn delete_storage(&mut self) {
        std::fs::remove_dir_all(self.0.storage_path()).unwrap();

        self.flush();
    }

    pub fn flush(&self) {
        if self.0.require_flushing() {
            self.0.perform_flush();
        }
    }
}

pub trait ConfigStorage {
    fn initialize(&self) -> Result<(), ConfigError>;

    fn root_path(&self) -> PathBuf;

    fn storage_path(&self) -> PathBuf;

    fn require_flushing(&self) -> bool {
        false
    }

    fn perform_flush(&self) {}
}

#[cfg(any(feature = "json", feature = "toml", feature = "yaml"))]
use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "json")]
impl<CS: ConfigStorage> StorageHolder<CS> {
    /// Create a field in the configuration and assigns it the value provided, serialized as a JSON.
    pub fn set_field_json<T: Serialize>(&mut self, path: impl AsRef<Path>, field: &T) -> Result<(), ConfigError> {
        Ok(self.write(path, serde_json::to_string(field).unwrap())?)
    }

    /// Provides the value of the field if it exists and deserializes it from a JSON.
    pub fn get_field_json<T: DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, ConfigError> {
        Ok(serde_json::from_str(&self.read_to_string(path).map_err(|_| ConfigError::FieldMissing)?).unwrap())
    }
}

#[cfg(feature = "toml")]
impl<CS: ConfigStorage> StorageHolder<CS> {
    /// Create a field in the configuration and assigns it the value provided, serialized as a TOML.
    pub fn set_field_toml<T: Serialize>(&mut self, path: impl AsRef<Path>, field: &T) -> Result<(), ConfigError> {
        Ok(self.write(path, serde_toml::ser::to_string_pretty(field).unwrap())?)
    }

    /// Provides the value of the field if it exists and deserializes it from a TOML.
    pub fn get_field_toml<T: DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, ConfigError> {
        Ok(serde_toml::de::from_str(&self.read_to_string(path).map_err(|_| ConfigError::FieldMissing)?).unwrap())
    }
}

#[cfg(feature = "yaml")]
impl<CS: ConfigStorage> StorageHolder<CS> {
    /// Create a field in the configuration and assigns it the value provided, serialized as a YAML.
    pub fn set_field_yaml<T: Serialize>(&mut self, path: impl AsRef<Path>, field: &T) -> Result<(), ConfigError> {
        Ok(self.write(path, serde_yaml::to_string(field).unwrap())?)
    }

    /// Provides the value of the field if it exists and deserializes it from a YAML.
    pub fn get_field_yaml<T: DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, ConfigError> {
        Ok(serde_yaml::from_str(&self.read_to_string(path).map_err(|_| ConfigError::FieldMissing)?).unwrap())
    }
}

// Mounts the debug save file on \"config:/\" if it isn't already, and then safely creates the directory for your plugin for the current user.

// It is heavily recommended to **NOT** manipulate \"config:/\" yourself, and instead use the returned ``ConfigStorage`` instance for safety reasons.
// pub fn acquire_storage<S, C>(plugin_name: &S) -> Result<StorageHolder<C>, ConfigError>
// where
//     S: AsRef<str> + ?Sized,
//     C: ConfigStorage,
// {
//     // for dir in std::fs::read_dir("config:/").unwrap() {
//     //     let dir = dir.unwrap();

//     //     println!("{}", dir.file_name().to_str().unwrap());
//     // }

//     Ok(DebugSavedataStorage(path).into())
// }
