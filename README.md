# mailing-list

mailing-list is an SMTP server with plugin support and an extensive configuration

## Configuration

The default location for the configuration is `/etc/mailing-list/daemon.toml`.

example `daemon.toml`:
```toml
hostname = "example.com"
port = 25

# Load plugins
plugins = [
    "libplugin.so",
]

# Dynamically load other list
[lists."members@example.com".Remote]
location = "members.toml"

# List directly in this file
[lists."board@example.com".Local]
members = ["foo@example.com", "bar@example.com"]

# If no defined users, send to another server
[forwarding]
enable = true
server = "[127.0.0.1]"
server_tls = "example.org"
port = 2525
```
members.toml:
```toml
[[medlemmar]]
namn = "Foo"
mail = "foo@example.com"

[[medlemmar]]
namn = "Bar"
mail = "bar@example.com"
```


