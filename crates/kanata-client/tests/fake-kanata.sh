#!/bin/sh
# A stand-in for `kanata` in the supervisor tests: no keyboard, no permissions.
#
#   FAKE_CHECK=fail          --check reports a config error (exit 1)
#   FAKE_START=permission    fails like a user without input/uinput access
#   FAKE_START=never-answer  runs but never opens its TCP port
#   FAKE_START=no-keyboard   logs "no keyboard devices found" and waits
#   FAKE_CRASH_AFTER=S       exits with code 3 after S seconds
case " $* " in
*" --check "*)
    if [ "$FAKE_CHECK" = fail ]; then
        echo "08:00:03.7792 [ERROR]   × Error in configuration"
        echo "  help: Unknown key/action: nosuchkey"
        exit 1
    fi
    echo "[INFO] config file is valid"
    exit 0
    ;;
esac

case "$FAKE_START" in
permission)
    echo "[ERROR] failed to open keyboard device(s): Permission denied (os error 13)" >&2
    exit 1
    ;;
never-answer) exec sleep 3600 ;;
no-keyboard)
    echo "[WARN] no keyboard devices found; kanata is waiting"
    exec sleep 3600
    ;;
esac

port=""
while [ $# -gt 0 ]; do
    [ "$1" = --port ] && port="${2##*:}"
    shift
done

exec python3 -u -c '
import os, socket, sys, threading
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
try:
    s.bind(("127.0.0.1", int(sys.argv[1])))
except OSError:
    print("TCP server starts: Os { code: 98, kind: AddrInUse, message: \"Address already in use\" }", file=sys.stderr)
    sys.exit(101)
s.listen()
print("\x1b[34m[INFO]\x1b[0m fake kanata ready")
crash = os.environ.get("FAKE_CRASH_AFTER")
if crash:
    threading.Timer(float(crash), lambda: os._exit(3)).start()
while True:
    c, _ = s.accept()
    c.sendall(b"{\"LayerChange\":{\"new\":\"base\"}}\n")
' "$port"
