#!/bin/sh
# A stand-in for `kanata` in the supervisor tests: no keyboard, no permissions.
#
#   FAKE_CHECK=fail          --check reports a config error (exit 1)
#   FAKE_START=permission    fails like a user without input/uinput access
#   FAKE_START=never-answer  runs but never opens its TCP port
#   FAKE_START=no-keyboard   logs "no keyboard devices found" and waits
#   FAKE_START=no-device     exits like Kanata when none of its devices exist
#   FAKE_CRASH_AFTER=S       exits with code 3 after S seconds
#   FAKE_KEYS="KEY_A:Press KEY_A:Release pause …"
#                            with --debug, prints these as Kanata's debug key
#                            lines every 100 ms (looping; "pause" waits 1 s),
#                            plus a non-key line
#   FAKE_TAPHOLD="caps:tap caps:hold"
#                            sends TapActivated / HoldActivated to every client,
#                            one per second, looping
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
no-device)
    echo "[ERROR] No keyboard devices were found. Try:" >&2
    echo "1. Run 'kanata --list' to see available devices" >&2
    exit 1
    ;;
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
debug=0
while [ $# -gt 0 ]; do
    [ "$1" = --port ] && port="${2##*:}"
    [ "$1" = --debug ] && debug=1
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
import json, time
keys = os.environ.get("FAKE_KEYS", "").split()
if sys.argv[2] == "1" and keys:
    def keys_loop():
        while True:
            for k in keys:
                if k == "pause":
                    time.sleep(1)
                    continue
                name, value = k.split(":")
                print("00:00:00.0000 \x1b[36m[DEBUG]\x1b[0m process recv ev KeyEvent { code: %s (0), value: %s }" % (name, value))
                time.sleep(0.1)
            print("00:00:00.0000 \x1b[36m[DEBUG]\x1b[0m layer stack: base")
    threading.Thread(target=keys_loop, daemon=True).start()
clients = []
taphold = os.environ.get("FAKE_TAPHOLD", "").split()
if taphold:
    def taphold_loop():
        while True:
            for t in taphold:
                key, kind = t.split(":")
                msg = (json.dumps({"TapActivated" if kind == "tap" else "HoldActivated": {"key": key}}) + "\n").encode()
                for c in list(clients):
                    try:
                        c.sendall(msg)
                    except OSError:
                        clients.remove(c)
                time.sleep(1)
    threading.Thread(target=taphold_loop, daemon=True).start()
while True:
    c, _ = s.accept()
    c.sendall(b"{\"LayerChange\":{\"new\":\"base\"}}\n")
    clients.append(c)
' "$port" "$debug"
