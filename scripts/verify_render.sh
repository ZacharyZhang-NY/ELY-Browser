#!/usr/bin/env bash
# T16 — real-window screencapture sanity check.
#
# Boots ./target/release/ely_app in the background, opens a live page through
# Servo's native surface path, captures the ELY window by its Core Graphics
# window ID, then asserts the PNG is non-trivial (size + dimensions + non-white
# center). Stderr from the app is
# tee'd to a log so a crash leaves evidence behind.
#
# Idempotent: kills any leftover ely_app processes from prior runs before
# starting, and cleans up its own background process on exit (success or fail).

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
SHOT_PATH="/tmp/ely-verify-${TIMESTAMP}.png"
STDERR_LOG="/tmp/ely-verify-stderr.log"
APP_BIN="${REPO_ROOT}/target/release/ely_app"
SMOKE_URL="${ELY_VERIFY_URL:-https://servo.org/}"
APP_PID=""

cleanup() {
    if [[ -n "${APP_PID}" ]] && kill -0 "${APP_PID}" 2>/dev/null; then
        kill "${APP_PID}" 2>/dev/null || true
        # give it a beat to exit cleanly, then SIGKILL if needed
        for _ in 1 2 3 4 5; do
            kill -0 "${APP_PID}" 2>/dev/null || break
            sleep 0.2
        done
        kill -9 "${APP_PID}" 2>/dev/null || true
    fi
    # Stragglers from prior runs / child processes
    pkill -f "target/release/ely_app" 2>/dev/null || true
    pkill -x "ely_app" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

fail() {
    echo "FAIL: $*" >&2
    if [[ -s "${STDERR_LOG}" ]]; then
        echo "---- stderr head (50 lines) ----" >&2
        head -n 50 "${STDERR_LOG}" >&2
        echo "--------------------------------" >&2
    fi
    exit 1
}

echo "[1/6] killing stale ely_app processes"
pkill -f "target/release/ely_app" 2>/dev/null || true
pkill -x "ely_app" 2>/dev/null || true
sleep 0.5

echo "[2/6] cargo build --release --locked -p ely_app"
if ! cargo build --release --locked -p ely_app; then
    fail "cargo build failed"
fi
[[ -x "${APP_BIN}" ]] || fail "binary missing: ${APP_BIN}"

echo "[3/6] launching ${APP_BIN} ${SMOKE_URL} (stderr -> ${STDERR_LOG})"
: > "${STDERR_LOG}"
"${APP_BIN}" "${SMOKE_URL}" >/dev/null 2>"${STDERR_LOG}" &
APP_PID=$!
echo "      pid=${APP_PID}"

echo "[4/6] waiting 12s for window + live page to settle"
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do
    sleep 1
    if ! kill -0 "${APP_PID}" 2>/dev/null; then
        fail "app exited early during warm-up (after ${i}s)"
    fi
done

echo "[5/6] activating pid ${APP_PID} and screencapture -> ${SHOT_PATH}"
if ! osascript >/dev/null <<OSA
tell application "System Events"
    set frontmost of first process whose unix id is ${APP_PID} to true
end tell
OSA
then
    fail "could not activate ely_app process ${APP_PID}"
fi
sleep 1
FRONTMOST=$(osascript -e \
    "tell application \"System Events\" to get frontmost of first process whose unix id is ${APP_PID}")
[[ "${FRONTMOST}" == "true" ]] || fail "ely_app process ${APP_PID} is not frontmost"

WINDOW_ID=$(/usr/bin/swift -e '
import CoreGraphics
import Darwin
import Foundation

guard CommandLine.arguments.count == 2, let targetPID = Int32(CommandLine.arguments[1]) else {
    exit(64)
}
let windows = CGWindowListCopyWindowInfo(
    [.optionOnScreenOnly, .excludeDesktopElements],
    kCGNullWindowID
) as? [[String: Any]] ?? []

let candidates = windows.compactMap { window -> (number: UInt32, area: Double)? in
    guard
        let owner = window[kCGWindowOwnerPID as String] as? NSNumber,
        owner.int32Value == targetPID,
        let layer = window[kCGWindowLayer as String] as? NSNumber,
        layer.intValue == 0,
        let number = window[kCGWindowNumber as String] as? NSNumber,
        let bounds = window[kCGWindowBounds as String] as? [String: Any],
        let width = bounds["Width"] as? NSNumber,
        let height = bounds["Height"] as? NSNumber,
        width.doubleValue > 100,
        height.doubleValue > 100
    else {
        return nil
    }

    return (number.uint32Value, width.doubleValue * height.doubleValue)
}

guard let window = candidates.max(by: { $0.area < $1.area }) else {
    exit(1)
}
print(window.number)
' "${APP_PID}")
[[ "${WINDOW_ID}" =~ ^[0-9]+$ ]] || fail "could not resolve ELY window for pid ${APP_PID}"
echo "      window_id=${WINDOW_ID}"

# -x silences the shutter sound; -l limits capture to the verified ELY window.
if ! screencapture -x -l "${WINDOW_ID}" "${SHOT_PATH}"; then
    fail "screencapture invocation failed"
fi
[[ -f "${SHOT_PATH}" ]] || fail "screencapture produced no file"

echo "[6/6] verifying screenshot"

# size > 10KB
BYTES=$(stat -f%z "${SHOT_PATH}")
echo "      size: ${BYTES} bytes"
if (( BYTES <= 10240 )); then
    fail "screenshot too small (${BYTES} bytes <= 10KB) — likely blank/failed"
fi

# dimensions > 100x100
PIX_W=$(sips -g pixelWidth  "${SHOT_PATH}" | awk '/pixelWidth:/  {print $2}')
PIX_H=$(sips -g pixelHeight "${SHOT_PATH}" | awk '/pixelHeight:/ {print $2}')
echo "      dims: ${PIX_W} x ${PIX_H}"
if [[ -z "${PIX_W}" || -z "${PIX_H}" ]]; then
    fail "could not read dimensions via sips"
fi
if (( PIX_W <= 100 || PIX_H <= 100 )); then
    fail "screenshot dimensions too small (${PIX_W}x${PIX_H})"
fi

# Center-region non-white check. Crops a center patch with sips (built-in on
# macOS), then decodes the resulting PNG via python3 stdlib only (zlib +
# struct) — no PyObjC / no new deps. Averages RGB; FAILs if all channels
# > 248, which would mean we captured an empty white desktop.
CENTER_PATCH="${SHOT_PATH%.png}-center.png"
PATCH_PX=128
PATCH_OFF_W=$(( PIX_W / 2 - PATCH_PX / 2 ))
PATCH_OFF_H=$(( PIX_H / 2 - PATCH_PX / 2 ))
if ! sips --cropToHeightWidth "${PATCH_PX}" "${PATCH_PX}" \
          --cropOffset "${PATCH_OFF_H}" "${PATCH_OFF_W}" \
          "${SHOT_PATH}" --out "${CENTER_PATCH}" >/dev/null 2>&1; then
    fail "sips crop failed"
fi

PY_OUT=$(/usr/bin/python3 - "${CENTER_PATCH}" <<'PYEOF'
import sys, struct, zlib

path = sys.argv[1]
with open(path, "rb") as f:
    data = f.read()
if data[:8] != b"\x89PNG\r\n\x1a\n":
    print("ERROR not-png"); sys.exit(3)

i = 8
width = height = bit_depth = color_type = None
idat = bytearray()
while i < len(data):
    length = struct.unpack(">I", data[i:i+4])[0]
    ctype = data[i+4:i+8]
    payload = data[i+8:i+8+length]
    i += 8 + length + 4  # skip crc
    if ctype == b"IHDR":
        width, height, bit_depth, color_type = struct.unpack(">IIBB", payload[:10])
    elif ctype == b"IDAT":
        idat.extend(payload)
    elif ctype == b"IEND":
        break

if bit_depth != 8 or color_type not in (2, 6):
    print(f"ERROR unsupported bit_depth={bit_depth} color_type={color_type}")
    sys.exit(3)

channels = 3 if color_type == 2 else 4
stride = width * channels
raw = zlib.decompress(bytes(idat))
# undo PNG row filters
out = bytearray(width * height * channels)
prev = bytearray(stride)
pos = 0
for y in range(height):
    f = raw[pos]; pos += 1
    row = bytearray(raw[pos:pos+stride]); pos += stride
    if f == 0:
        pass
    elif f == 1:  # Sub
        for x in range(channels, stride):
            row[x] = (row[x] + row[x-channels]) & 0xFF
    elif f == 2:  # Up
        for x in range(stride):
            row[x] = (row[x] + prev[x]) & 0xFF
    elif f == 3:  # Average
        for x in range(stride):
            left = row[x-channels] if x >= channels else 0
            row[x] = (row[x] + (left + prev[x]) // 2) & 0xFF
    elif f == 4:  # Paeth
        for x in range(stride):
            a = row[x-channels] if x >= channels else 0
            b = prev[x]
            c = prev[x-channels] if x >= channels else 0
            p = a + b - c
            pa, pb, pc = abs(p-a), abs(p-b), abs(p-c)
            pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
            row[x] = (row[x] + pr) & 0xFF
    else:
        print(f"ERROR filter={f}"); sys.exit(3)
    out[y*stride:(y+1)*stride] = row
    prev = row

rs = gs = bs = n = 0
for y in range(height):
    base = y * stride
    for x in range(width):
        o = base + x * channels
        rs += out[o]; gs += out[o+1]; bs += out[o+2]; n += 1
ar, ag, ab = rs/n, gs/n, bs/n
print(f"CENTER {width}x{height} r={ar:.1f} g={ag:.1f} b={ab:.1f}")
if ar > 248 and ag > 248 and ab > 248:
    sys.exit(2)
sys.exit(0)
PYEOF
)
PY_RC=$?
echo "      center: ${PY_OUT}"
rm -f "${CENTER_PATCH}"
if (( PY_RC == 2 )); then
    fail "center ${PATCH_PX}x${PATCH_PX} patch is ~white — likely empty desktop or failed paint"
fi
if (( PY_RC != 0 )); then
    fail "screenshot decoder rejected the center patch (exit ${PY_RC})"
fi

echo
echo "PASS"
echo "screenshot: ${SHOT_PATH}"
echo "stderr log: ${STDERR_LOG} ($(wc -l < "${STDERR_LOG}" | tr -d ' ') lines)"
exit 0
