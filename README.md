# mcumgr-client

This is a Rust program to run mcumgr commands, used for example for Zephyr, for uploading firmware updates from a PC to an embedded device. It is a faster alternative to [the mcumgr Go program](https://github.com/apache/mynewt-mcumgr-cli).

## Download

Released builds for x86-64 Windows, Linux, and MacOS are [here](https://github.com/vouch-opensource/mcumgr-client/releases).

Example download:
```
wget https://github.com/vouch-opensource/mcumgr-client/releases/latest/download/mcumgr-client-linux-x86.zip
wget -O - https://github.com/vouch-opensource/mcumgr-client/releases/latest/download/mcumgr-client-linux-x86.zip.sha256sum | sha256sum --check
unzip mcumgr-client-linux-x86.zip
```

## Build Dependencies

Install Rust:

Recommended is with [rustup](https://www.rust-lang.org/tools/install), because then it is easy to keep it up to date.

## Build
Change to this directory and build it:
```
cargo build --release
```
Without `--release`, it builds in debug mode.

## Transport Options

mcumgr-client supports two transport methods:

### Serial Transport (default)
Use `-d` or `--device` to specify a serial port:
```bash
mcumgr-client -d /dev/ttyACM0 <command>
```

If not specified and only one serial device exists, it will be used automatically.

### UDP Transport
Use `--host` to connect over UDP (SMP over UDP):
```bash
mcumgr-client --host 192.0.2.1 <command>
```

The default UDP port is 1337. Use `--port` to specify a different port:
```bash
mcumgr-client --host 192.0.2.1 --port 1338 <command>
```

## Commands

### Image Management

**List images:**
```bash
mcumgr-client -d /dev/ttyACM0 list
mcumgr-client --host 192.0.2.1 list
```

**Upload firmware:**
```bash
mcumgr-client -d /dev/ttyACM0 upload firmware-image.bin
mcumgr-client --host 192.0.2.1 upload firmware-image.bin

# Upload to a specific slot
mcumgr-client -d /dev/ttyACM0 upload firmware-image.bin --slot 1
```

**Test/confirm an image:**
```bash
# Mark image for test boot
mcumgr-client -d /dev/ttyACM0 test <image-hash>

# Confirm the current image
mcumgr-client -d /dev/ttyACM0 test <image-hash> --confirm true
```

**Erase an image slot:**
```bash
mcumgr-client -d /dev/ttyACM0 erase
mcumgr-client -d /dev/ttyACM0 erase --slot 1
```

### OS/Device Management

**Reset the device:**
```bash
mcumgr-client -d /dev/ttyACM0 reset
mcumgr-client --host 192.0.2.1 reset
```

**Echo test:**
```bash
mcumgr-client --host 192.0.2.1 echo "hello world"
```

**Get task/thread statistics:**
```bash
mcumgr-client --host 192.0.2.1 taskstat
```

**Get MCUmgr parameters:**
```bash
mcumgr-client --host 192.0.2.1 mcumgr-params
```

**Get OS/application information:**
```bash
mcumgr-client --host 192.0.2.1 os-info
mcumgr-client --host 192.0.2.1 os-info --format a   # all info
mcumgr-client --host 192.0.2.1 os-info --format s   # kernel name
mcumgr-client --host 192.0.2.1 os-info --format v   # kernel version
```

Format specifiers: `s`=kernel name, `n`=node name, `r`=release, `v`=version, `b`=build date, `m`=machine, `p`=processor, `i`=platform, `o`=OS, `a`=all

**Get bootloader information:**
```bash
mcumgr-client --host 192.0.2.1 bootloader-info
mcumgr-client --host 192.0.2.1 bootloader-info --query mode
```

**Get hardware ID:**
```bash
mcumgr-client --host 192.0.2.1 hwid
```
Note: Requires custom os-info hook on the device supporting format `h`.

### Shell Management

Execute shell commands on the device (requires `CONFIG_MCUMGR_GRP_SHELL=y`):

```bash
mcumgr-client --host 192.0.2.1 shell "kernel uptime"
mcumgr-client --host 192.0.2.1 shell "kernel version"
mcumgr-client --host 192.0.2.1 shell "device list"
mcumgr-client --host 192.0.2.1 shell "sensor get bmi088@0"
```

### File System Management

Requires `CONFIG_MCUMGR_GRP_FS=y` on the device.

**Download a file from the device:**
```bash
mcumgr-client --host 192.0.2.1 fs-download /lfs/config.txt ./config.txt
```

**Upload a file to the device:**
```bash
mcumgr-client --host 192.0.2.1 fs-upload ./config.txt /lfs/config.txt
```

**Get file size:**
```bash
mcumgr-client --host 192.0.2.1 fs-stat /lfs/config.txt
```

**Calculate file hash:**
```bash
mcumgr-client --host 192.0.2.1 fs-hash /lfs/config.txt
mcumgr-client --host 192.0.2.1 fs-hash /lfs/config.txt --hash-type crc32
```

### Statistics Management

Requires `CONFIG_MCUMGR_GRP_STAT=y` on the device.

**List available statistics groups:**
```bash
mcumgr-client --host 192.0.2.1 stat-list
```

**Read statistics from a group:**
```bash
mcumgr-client --host 192.0.2.1 stat-read mygroup
```

### Settings/Config Management

Requires `CONFIG_MCUMGR_GRP_SETTINGS=y` on the device.

**Read a setting:**
```bash
mcumgr-client --host 192.0.2.1 settings-read my/setting/key
mcumgr-client --host 192.0.2.1 settings-read my/setting/key --max-size 256
```

**Write a setting (hex value):**
```bash
mcumgr-client --host 192.0.2.1 settings-write my/setting/key 48656c6c6f
```

**Delete a setting:**
```bash
mcumgr-client --host 192.0.2.1 settings-delete my/setting/key
```

**Commit/load/save settings:**
```bash
mcumgr-client --host 192.0.2.1 settings-commit
mcumgr-client --host 192.0.2.1 settings-load
mcumgr-client --host 192.0.2.1 settings-save
```

## Global Options

| Option | Description | Default |
|--------|-------------|---------|
| `-d, --device` | Serial port device | Auto-detect |
| `--host` | UDP host (use instead of serial) | - |
| `--port` | UDP port | 1337 |
| `-v, --verbose` | Enable debug logging | false |
| `-t, --initial_timeout` | Initial timeout in seconds | 60 |
| `-u, --subsequent_timeout` | Subsequent timeout in ms | 200 |
| `--nb_retry` | Number of retries per packet | 4 |
| `-l, --linelength` | Maximum line length (serial) | 128 |
| `-m, --mtu` | Maximum request size | 512 |
| `-b, --baudrate` | Serial baud rate | 115200 |

## Zephyr Configuration

To enable MCUmgr features on your Zephyr device, add the relevant Kconfig options:

### Basic MCUmgr (required)
```
CONFIG_MCUMGR=y
CONFIG_MCUMGR_TRANSPORT_UDP=y        # For UDP transport
CONFIG_MCUMGR_TRANSPORT_UDP_IPV4=y
```

### OS Management Group
```
CONFIG_MCUMGR_GRP_OS=y
CONFIG_MCUMGR_GRP_OS_INFO=y
CONFIG_MCUMGR_GRP_OS_TASKSTAT=y
CONFIG_MCUMGR_GRP_OS_MCUMGR_PARAMS=y
CONFIG_MCUMGR_GRP_OS_BOOTLOADER_INFO=y
```

### Image Management Group
```
CONFIG_MCUMGR_GRP_IMG=y
```

### Shell Management Group
```
CONFIG_SHELL=y
CONFIG_SHELL_BACKEND_DUMMY=y
CONFIG_BASE64=y
CONFIG_MCUMGR_GRP_SHELL=y
```

### Statistics Management Group
```
CONFIG_STATS=y
CONFIG_STATS_NAMES=y
CONFIG_MCUMGR_GRP_STAT=y
```

### File System Management Group
```
CONFIG_FILE_SYSTEM=y
CONFIG_MCUMGR_GRP_FS=y
```

### Settings Management Group
```
CONFIG_SETTINGS=y
CONFIG_MCUMGR_GRP_SETTINGS=y
```

## Examples

**Flash firmware over serial with optimized settings:**
```bash
mcumgr-client -m 4096 -l 8192 -d /dev/ttyACM0 upload firmware-image.bin
```

**Monitor device over UDP:**
```bash
mcumgr-client --host 192.0.2.1 os-info
mcumgr-client --host 192.0.2.1 taskstat
mcumgr-client --host 192.0.2.1 shell "kernel uptime"
```

**Auto-detect serial device:**
You can omit the `-d` parameter. If only one device exists, it will be used automatically. If the filename contains `slot1` or `slot3`, it flashes to that slot:
```bash
mcumgr-client upload firmware-slot1.bin
mcumgr-client upload ext-flash-slot3.bin
```

# Notes
There is a bug in the Zephyr CDC ACM driver. When building mcuboot for it, it needs this patch:

```
--- a/subsys/usb/device/class/cdc_acm.c
+++ b/subsys/usb/device/class/cdc_acm.c
@@ -70,7 +70,7 @@ LOG_MODULE_REGISTER(usb_cdc_acm, CONFIG_USB_CDC_ACM_LOG_LEVEL);
 #define CDC_ACM_DEFAULT_BAUDRATE {sys_cpu_to_le32(115200), 0, 0, 8}
 
 /* Size of the internal buffer used for storing received data */
-#define CDC_ACM_BUFFER_SIZE (CONFIG_CDC_ACM_BULK_EP_MPS)
+#define CDC_ACM_BUFFER_SIZE 512
 
 /* Serial state notification timeout */
 #define CDC_CONTROL_SERIAL_STATE_TIMEOUT_US 100000
```

With the default settings for the line length and MTU, it needs about 3 minutes to flash 917,504 bytes (5 times faster than the original mcumgr program). With these settings for mcuboot:

```
CONFIG_BOOT_MAX_LINE_INPUT_LEN=8192
CONFIG_BOOT_SERIAL_MAX_RECEIVE_SIZE=4096
```

The line length and MTU size can be increased, like this for the example:

```
./target/release/mcumgr-client -m 4096 -l 8192 -d /dev/ttyACM0 upload firmware-image.bin 
```

This needs 17 seconds for the same file (instead of 1:48 minutes with the default buffer sizes), which is more than 10 times faster than the original mcumgr Go program.

# Python wrapper
To make it easier to use the program from Python, there is a wrapper for it [here](https://pypi.org/project/mcumgr-client-wrapper/).
