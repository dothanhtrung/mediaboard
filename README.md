# Media Board

I need a tool to manage my photos and videos on my Raspberry Pi, so I made one with Rust and SQLite.

Support tag, too.

## Build

### Install dependencies

```shell
# Debian
$ sudo apt install libsqlite3-dev
```

```shell
# Fedora
$ sudo dnf install sqlite-devel
```

### Build

```shell
$ cargo build --release
```

## Run

### Create database

```shell
$ sqlite3 mediaboard.db -init ./mediaboard.sql
```

### Update config

```ini
# config.ini
[DEFAULT]
# Web port
port = 8088

# Folder that stores media
root = path/to/media/folder

# Path to db file
db = path/to/mediaboard.db

# Items per page
ipp = 24
```

### Run

```shell
$ ./target/release/mediaboard
```

Now the website is available at http://127.0.0.1:8088.