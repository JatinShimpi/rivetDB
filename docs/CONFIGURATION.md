# RivetDB Configuration Guide

## Overview

RivetDB uses TOML for configuration and supports both configuration files and command-line arguments. CLI arguments override configuration file values.

## Configuration File

Default location: `rivetdb.toml`

```toml
[server]
host = "127.0.0.1"
port = 7878
max_connections = 10000

[memory]
max_memory = "1GB"
eviction_policy = "allkeys-lfu"

[persistence]
aof_enabled = true
aof_fsync = "everysec"
aof_filename = "appendonly.aof"

[logging]
level = "info"
```

## Configuration Sections

### [server]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `host` | string | `"127.0.0.1"` | Server bind address |
| `port` | integer | `7878` | Server bind port |
| `max_connections` | integer | `10000` | Maximum concurrent connections |

### [memory]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `max_memory` | string | `"64MB"` | Maximum memory usage (supports KB, MB, GB) |
| `eviction_policy` | string | `"allkeys-lfu"` | Eviction policy when memory limit reached |

**Eviction Policies:**
- `allkeys-lfu` - Evict least frequently used keys
- `allkeys-lru` - Evict least recently used keys
- `noeviction` - Return errors when memory limit reached

### [persistence]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `aof_enabled` | boolean | `false` | Enable AOF persistence |
| `aof_fsync` | string | `"everysec"` | fsync frequency |
| `aof_filename` | string | `"appendonly.aof"` | AOF file name |

**fsync Policies:**
- `always` - fsync after every write (slow, safest)
- `everysec` - fsync every second (balanced)
- `no` - Let OS decide (fast, least safe)

### [logging]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `level` | string | `"info"` | Log level |

**Log Levels:** `trace`, `debug`, `info`, `warn`, `error`

## Command-Line Arguments

```bash
rivetdb --help
```

### Options

| Flag | Long | Description |
|------|------|-------------|
| `-c` | `--config <FILE>` | Configuration file path (default: `rivetdb.toml`) |
| | `--host <HOST>` | Override server host |
| `-p` | `--port <PORT>` | Override server port |
| | `--log-level <LEVEL>` | Override log level |
| `-h` | `--help` | Print help information |
| `-V` | `--version` | Print version information |

## Usage Examples

### Default Configuration

```bash
rivetdb
```

Loads `rivetdb.toml` from current directory.

### Custom Configuration File

```bash
rivetdb --config /path/to/custom.toml
```

### Override Port

```bash
rivetdb --port 6379
```

### Different Log Level

```bash
rivetdb --log-level debug
```

### Combine Multiple Options

```bash
rivetdb --config prod.toml --port 6379 --log-level warn
```

### Environment Variable for Log Level

```bash
RUST_LOG=debug rivetdb
```

The `RUST_LOG` environment variable takes precedence over config file and CLI arguments.

## Log Output

### Structured Logging

RivetDB uses `tracing` for structured logging:

```
2026-01-19T01:30:00.123Z  INFO rivetdb: Starting RivetDB version=0.1.0
2026-01-19T01:30:00.124Z  INFO rivetdb: Server configuration host=127.0.0.1 port=7878
2026-01-19T01:30:00.125Z  INFO rivetdb: Memory configuration max_memory=1GB eviction_policy=allkeys-lfu
2026-01-19T01:30:00.126Z  INFO rivetdb: Server listening addr=127.0.0.1:7878
2026-01-19T01:30:01.234Z  INFO rivetdb: New connection client=127.0.0.1:54321
2026-01-19T01:30:01.235Z DEBUG rivetdb: Processing command client=127.0.0.1:54321 command=SET args=["key", "value"]
```

### Log Levels in Practice

**trace**: Very verbose, everything  
**debug**: Command processing details  
**info**: Startup, connections, config (recommended)  
**warn**: Slow commands, deprecated features  
**error**: Errors, panics, system failures  

## Production Recommendations

### Recommended Production Config

```toml
[server]
host = "0.0.0.0"  # Listen on all interfaces
port = 7878
max_connections = 10000

[memory]
max_memory = "4GB"  # Adjust based on available RAM
eviction_policy = "allkeys-lfu"

[persistence]
aof_enabled = true
aof_fsync = "everysec"

[logging]
level = "info"  # Don't use debug in production
```

### Security Notes

- **Never expose to public internet** - Use firewall or VPN
- **Bind to specific interface** - Don't use `0.0.0.0` on public servers
- **Monitor logs** - Watch for unusual patterns
- **Set memory limits** - Prevent OOM

## Configuration Generation

Generate default config file:

```bash
rivetdb --help  # Shows default values
```

Create `rivetdb.toml` manually with desired settings.

## Troubleshooting

### Server won't start

- **Check port availability**: Another process using the port?
  ```bash
  netstat -an | findstr :7878  # Windows
  lsof -i :7878                # Linux/Mac
  ```

- **Check bind address**: Can only bind to local interfaces
- **Check logs**: Run with `--log-level debug`

### Memory issues

- Adjust `max_memory` in config
- Change eviction policy if getting too many evictions
- Monitor with `STATS` and `MEMORY` commands

### Performance issues

- Increase `max_connections` if needed
- Use `SLOWLOG` command to identify slow operations
- Consider changing `aof_fsync` to `no` (testing only!)
