use std::{fs::ReadDir, io, path::Path, str::FromStr};

use skyline::nn;
use thiserror::Error;

extern "C" {
    #[link_name = "\u{1}_ZN2nn2fs21MountSaveDataForDebugEPKc"]
    pub fn MountSaveDataForDebug(arg1: *const u8) -> i32;
    #[link_name = "\u{1}_ZN2nn2fs6CommitEPKc"]
    pub fn SaveDataCommit(arg1: *const u8) -> i32;
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

/// Abstraction over the configuration directory created for your plugin for the current user.
///
/// It is heavily recommended to **NOT** manipulate \"config:/\" yourself, and instead use the methods implemented on ConfigStorage for safety reasons.
pub struct ConfigStorage(std::path::PathBuf);

impl ConfigStorage {
    /// TODO: Rework this to allow copying the config from one user to the other using the UID to compute paths.
    fn copy<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> io::Result<u64> {
        todo!();

        let full_path_from = self.0.join(from);
        let full_path_to = self.0.join(to);

        std::fs::copy(full_path_from, full_path_to).map(|res| {
            self.flush();
            res
        })
    }

    fn create<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let full_path = self.0.join(path);

        std::fs::File::create(full_path).map(|_| ())
    }

    fn remove_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let full_path = self.0.join(path);

        std::fs::remove_file(full_path)?;
        self.flush();
        Ok(())
    }

    /// Renames a field or a flag to another name.
    pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(&mut self, from: P, to: Q) -> Result<(), ConfigError> {
        let full_path_from = self.0.join(from);
        let full_path_to = self.0.join(to);

        std::fs::rename(full_path_from, full_path_to)?;
        self.flush();
        Ok(())
    }

    fn read_to_string<P: AsRef<Path>>(&self, path: P) -> io::Result<String> {
        let full_path = self.0.join(path);

        std::fs::read_to_string(full_path)
    }

    /// Abstraction of ``std::fs::read_dir`` over the Configuration Storage.
    pub fn read_dir(&self) -> io::Result<ReadDir> {
        std::fs::read_dir(&self.0)
    }

    fn write<P: AsRef<Path>, C: AsRef<[u8]>>(&mut self, path: P, contents: C) -> Result<(), ConfigError> {
        let full_path = self.0.join(path);

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
        let full_path = self.0.join(path);
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

    pub fn flush(&self) {
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

#[cfg(any(feature = "json", feature = "toml", feature = "yaml"))]
use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "json")]
impl ConfigStorage {
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
impl ConfigStorage {
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
impl ConfigStorage {
    /// Create a field in the configuration and assigns it the value provided, serialized as a YAML.
    pub fn set_field_yaml<T: Serialize>(&mut self, path: impl AsRef<Path>, field: &T) -> Result<(), ConfigError> {
        Ok(self.write(path, serde_yaml::to_string(field).unwrap())?)
    }

    /// Provides the value of the field if it exists and deserializes it from a YAML.
    pub fn get_field_yaml<T: DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, ConfigError> {
        Ok(serde_yaml::from_str(&self.read_to_string(path).map_err(|_| ConfigError::FieldMissing)?).unwrap())
    }
}

/// Mounts the debug save file on \"config:/\" if it isn't already, and then safely creates the directory for your plugin for the current user.
///
/// It is heavily recommended to **NOT** manipulate \"config:/\" yourself, and instead use the returned ``ConfigStorage`` instance for safety reasons.
pub fn acquire_storage<S>(plugin_name: &S) -> Result<ConfigStorage, ConfigError>
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
