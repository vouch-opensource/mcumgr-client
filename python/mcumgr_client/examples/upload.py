#!/usr/bin/env python3

import sys
import mcumgr_client as mcgr

try:
    s = mcgr.Session(sys.argv[1], 576000)
    d = s.list()
    print(d)

    s.upload(sys.argv[2])

    d = s.list()
    print(d)

    s.reset()
except mcgr.CalledProcessError as e:
    print(e)
    print("--- STDERR [")
    print(e.stderr)
    print("] STDERR ---")
