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

## Run
List existing images:
```
./target/release/mcumgr-client -d /dev/ttyACM0 list
```

Example to flash a firmware image:
```
./target/release/mcumgr-client -d /dev/ttyACM0 upload firmware-image.bin 
```

Example to flash an external flash in slot 3, and with the increased MTU and line length settings as explained in the notes:
```
./target/release/mcumgr-client -s 3 -m 4096 -l 8192 -d /dev/ttyACM0 upload ext-flash.bin 
```

Example to rest a device:
```
./target/release/mcumgr-client -d /dev/ttyACM0 reset
```

You can omit the `-d` parameter for the device. If not specified and there are more than one device, it lists all detected devices. If there is only one device, it uses this device, if no device name is specified. And if the filename contains `slot1`, for example `firmware-slot1.bin`, then it flashes to slot 1. If it contains `slot3`, then it flashes to slot 3. This makes updates fail-safe and easy to do. For example you can use it like this with the right file names:
```
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
