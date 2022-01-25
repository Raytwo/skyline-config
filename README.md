# skyline-config

Utility crate to manage configuration for Skyline plugins

### Basic library usage

```rust
// Get a ConfigStorage interface to store configuration files for the current user.
// While untested, you should be able to acquire multiple storages at the same time if they have different names (even if they don't, but that'd be silly).
let config_storage = skyline_config::::acquire_storage("arcropolis").unwrap();
```

From there, you can use various methods on the ``ConfigStorage`` to manipulate your configuration fields and flags.

set_flag:
```rust
storage.set_flag("beta_updates", true);
```

get_flag:
```rust
let uses_beta_updates: bool = storage.get_flag("beta_updates");
```

set_field:
```rust
storage.set_field("logging_level", "Info").unwrap();
...
storage.set_field("max_threshold", 69).unwrap();
```

get_field (use type inference to specify what type it should deserialize to):
```rust
let logging_level: String = storage.get_field("logging_level").unwrap();
```

Starting from 0.2.0, methods have been added to (de)serialize from JSON, TOML and YAML by activating the desired feature flag in your Cargo.toml
```rust
storage.set_field_json("config", &config).unwrap();
let deserialized_config: Config = storage.get_field_json("config").unwrap();
```

ReadDir:
```rust
storage.read_dir().unwrap().into_iter().for_each(|dir| {
    println!("{:?}", dir.unwrap().path())
});
```

ClearStorage (this deletes everything in the configuration. Use with care):
```rust
storage.clear_storage();

```


Things to note:

1. Each user gets their own configuration file through the usage of their Uid. This means that if the user delete their current profile, the configuration is lost except if you back it up for them. There are plans to be able to mount a ConfigurationStorage with a provided Uid in the near future.
2. The debug save data is mounted on ``config:/``. It is heavily recommended **NOT** to manipulate files yourself if you do not know what you are doing. Use ``ConfigStorage`` instead.
3. As the debug sava data is journalized, it requires flushing to reflect the changes, which is taken care of by ``ConfigStorage`` automatically. If you wish to submit a PR or edit the crate for your own needs, keep this in mind.
