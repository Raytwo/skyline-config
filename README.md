# skyline-config

Utility crate to facilitate the storage of configuration files for Skyline plugins

### Basic library usage

```rust
// Get a ConfigStorage interface to store configuration files for the current user.
// While untested, you should be able to acquire multiple storages at the same time if they have different names (even if they don't, but that'd be silly).
let config_storage = skyline_config::::acquire_storage("arcropolis").unwrap();
```

From there, you can use most std::fs I/O methods on the ``ConfigStorage`` to manipulate your configuration directory.

ReadDir:
```rust
storage.read_dir().unwrap().into_iter().for_each(|dir| {
    println!("{:?}", dir.unwrap().path())
});
```

Write:
```rust
storage.write("config.toml", b"Example").unwrap();
storage.write("subdir/config.toml", b"Example 2").unwrap();

```

While less recommended, you can also obtain the equivalent of ``std::fs::File`` in the form of ``ConfigFile`` by using ``create`` or ``open``, however it is less recommended as the storage requires flushing to reflect the changes. Only use this if you do not need to see the changes immediately.

Open:
```rust
let config_file = storage.open("config.toml").unwrap();

```

Things to note:

1. Each user gets their own configuration file through the usage of their Uid. This means that if the user delete their current profile, the configuration is lost except if you back it up for them.
2. The debug save data is mounted on ``config:/``. It is heavily recommended **NOT** to manipulate files yourself if you do not know what you are doing. Use ``ConfigStorage`` instead.
3. As the debug sava data is journalized, it requires flushing to reflect the changes, which is taken care of by ``ConfigStorage`` automatically.  
However, ``ConfigFile`` cannot implement this behavior because all files with Write permissions need to be closed before flushing.  
If you want to see the changes immediately after performing them, consider using ``ConfigStorage`` instead.
