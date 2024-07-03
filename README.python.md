# mcumgr-client

A python wrapper for the rust program [mcumgr-client](https://github.com/vouch-opensource/mcumgr-client).
This allows sending MCUmgr commands to a device connected to a serial port from Python.

# How to use

```
import mcumgr_client

s = mcumgr_client.Session(device='/dev/ttyUSB0', baudrate=576000)
# Get a dictionnary of properties 
d = s.list()
print(d)

# Upload image to device
s.upload('/path/to/image/bin')

# Reset the device
s.reset()
```

see `help(mcumgr_client)` for more
