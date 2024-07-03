"""mcumgr_client

A module to invoke mcumgr-client binary from python

Example:
    List the images on a device connected on ttyUSB0 at 576kbps::

        import mcumgr_client as mcgr

        s = mcgr.Session('/dev/ttyUSB0', 576000)
        properties = s.list()

Attributes:
    EXE (str): the name of the executable called to invoke mcumgr-client
"""

from subprocess import run, CompletedProcess, CalledProcessError
import re
import json
import os
from typing import Union

# Name of the MCUmgr client CLI program invoked by this wrapper
EXE = 'mcumgr-client'

# Regex to filter out escape sequences for the terminal
_ANSI_ESC_RE =  re.compile(r'\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])')

def _filter_out_escape(string: str) -> str:
    """Return a copy of the provided string without ANSI escape sequences

    Args:
        string (str): String to be filtered out

    Returns:
        str: filtered out string
    """
    return _ANSI_ESC_RE.sub('', string)

class Session:
    """A session allows sending MCUmgr commands to a device over a serial port.

    The configuration of the serial port can be set during initialization of the Session or
    explicitely passed to each command.

    A command can receive arguments in its **kwargs. The keys are transformed into options passed
    down to mcumgr-client. For example::

        Session().list(device='/dev/ttyUSB0', baudrate=576000)

    The supported **kwargs kay and type are defined in Session.ALLOWED_KWARGS

    Raises:
        RuntimeError: If no serial device was given, either during instanciation or command
        invocation.
        TypeError: If a kwargs cannot be converted into the expected type.
        CalledProcessError: If the wrapped mcumgr-client returned a non-zero code.

    Returns:
        _type_: _description_
    """

    ALLOWED_KWARGS = {
        'device': str,
        'slot': int,
        'verbose': bool,
        'initial_timeout': int,
        'subsequent_timeout': int,
        'linelength': int,
        'mtu': int,
        'baudrate': int,
    }

    def __init__(self, device: str=None, baudrate: int=None):
        """Instanciate a new session.

        A session is a set of parameters used for every invocation of mcumgr-client.

        Args:
            device (str, optional): Name of the device used for serial communication (/dev/ttyUSBx,
            COMx, etc.). Defaults to None, in which case, the device name must be explicitely set
            in a command's **kwargs.
            baudrate (int, optional): Baudrate of the serial device. Defaults to None, in which
            case, the default baudrate of mcumgr-client is used.
        """
        self._dev = device
        self._baudrate = baudrate

    def _generate_opts(self, opts: dict[str, any]):
        stropts = []

        if 'device' not in opts:
            if self._dev is None:
                raise RuntimeError("unspecified serial device")
            # use session value
            opts['device'] = self._dev

        if 'baudrate' not in opts and self._baudrate is not None:
            opts['baudrate'] = self._baudrate

        for k,v in opts.items():
            if k not in self.ALLOWED_KWARGS:
                # ignore args that are not options for mcumgr-client
                continue
            if isinstance(v, self.ALLOWED_KWARGS[k]):
                raise TypeError(f"expected {self.ALLOWED_KWARGS[k]} for {k}, received {type(v)}")
            stropts += [f"--{k}"] + [str(v)]

        return stropts

    def list(self, **kwargs) -> dict:
        """List the properties of the images in a device.

        Returns:
            dict: A dictionnary containing the properties of the listed images
        """

        kwargs.setdefault('initial_timeout', 1)
        res = self.run('list', **kwargs)

        string = _filter_out_escape(res.stdout)
        matchstr = 'response: '
        jsonstr = string[string.index(matchstr) + len(matchstr):]

        d = json.loads(jsonstr)

        for image in d['images']:
            image['hash'] = bytes(image['hash'])

        return d

    def reset(self, **kwargs):
        """Reset a device."""
        kwargs.setdefault('initial_timeout', 1)
        self.run('reset', **kwargs)

    def upload(self, image: Union[str, bytes, os.PathLike], **kwargs):
        """Upload an image on a device.

        Args:
            image (Union[str, bytes, os.PathLike]): Image to be uploaded on the device.
        """

        kwargs.setdefault('initial_timeout', 1)
        self.run('upload', str(image), **kwargs)

    def run(self, cmd: str, *args, **kwargs) -> CompletedProcess:
        """Run `cmd` on mcumgr-client with options passed through **kwargs and positional arguments
        passed through *args.

        Args:
            cmd (str): Name of the command invoked in mcumgr-client.

        Returns:
            CompletedProcess: Result of the operation.
        """
        res = run(
            [EXE]
            + self._generate_opts(kwargs)
            + [cmd]
            + [str(a) for a in args],
            capture_output=True, text=True, check = False)

        # We don't rely on run(check=True) to raise the error because we want
        # to filter out ANSI escape codes.
        if res.returncode != 0:
            raise CalledProcessError(res.returncode, res.args,
                                     _filter_out_escape(res.stdout),
                                     _filter_out_escape(res.stderr))

        return res
