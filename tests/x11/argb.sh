#!/bin/bash
# Testes do visual com alfa das camadas X11 (MIDE_X11_ARGB=1), em um display de teste.
# Sem compositor, o kit cai para 24 bits e avisa; com um compositor que reivindica
# _NET_WM_CM_S0 (xcompmgr), a janela sai em 32 bits e o alfa é aplicado.
#
# Uso: tests/x11/argb.sh [:66]
set -u
DISP="${1:-:66}"
case "$DISP" in
  :0|:0.*|:99|:99.*|:65|:65.*) echo "recusado: $DISP não é display de teste livre"; exit 2 ;;
esac
DIR="$(cd "$(dirname "$0")/../.." && pwd)"
EX="$DIR/target/debug/examples/x11-argb"
OUT="$(mktemp -d)"
PASS=0; FAIL=0
ok()  { echo "  ok    $1"; PASS=$((PASS+1)); }
nok() { echo "  FALHA $1"; FAIL=$((FAIL+1)); }

cleanup() {
  [ -n "${PID_EX:-}" ] && kill "$PID_EX" 2>/dev/null
  [ -n "${PID_CM:-}" ] && kill "$PID_CM" 2>/dev/null
  [ -n "${PID_XV:-}" ] && kill "$PID_XV" 2>/dev/null
  rm -rf "$OUT"
}
trap cleanup EXIT

if [ ! -x "$EX" ]; then
  echo "falta $EX (rode: cargo build --example x11-argb -j 2)"; exit 2
fi
if DISPLAY="$DISP" xdpyinfo >/dev/null 2>&1; then
  echo "recusado: $DISP já está em uso"; exit 2
fi

Xvfb "$DISP" -screen 0 1366x768x24 -nolisten tcp >/dev/null 2>&1 &
PID_XV=$!
sleep 2
export DISPLAY="$DISP"
unset WAYLAND_DISPLAY

# Roda o programa e devolve a profundidade e a cor no meio da metade transparente/semi.
rodar() { # rodar NOME
  MIDE_X11_ARGB=1 "$EX" > "$OUT/$1.txt" 2> "$OUT/$1.err" &
  local pid=$!
  for _ in $(seq 1 100); do grep -q PRONTO "$OUT/$1.txt" 2>/dev/null && break; sleep 0.1; done
  sleep 0.5
  local wid
  wid="$(xwininfo -root -children | awk '/mide-argb-teste/ {print $1; exit}')"
  [ -n "$wid" ] && xwininfo -id "$wid" | awk '/Depth:/ {print $2}' > "$OUT/$1.prof"
  xwd -root -silent 2>/dev/null | magick xwd:- "$OUT/$1.png" 2>/dev/null
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
}
pixel() { # pixel ARQUIVO.xwd X Y -> "r g b"
  python3 -I -c "
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert('RGB')
print(*im.getpixel((int(sys.argv[2]), int(sys.argv[3]))))" "$1" "$2" "$3"
}

echo "== sem compositor"
rodar semcm
check_sem() { grep -q '^24$' "$OUT/semcm.prof" && grep -q 'ignorado, sem compositor' "$OUT/semcm.err"; }
if check_sem; then ok "sem compositor: 24 bits e aviso"; else nok "sem compositor: 24 bits e aviso"; fi

echo "== com compositor (xcompmgr)"
xcompmgr >/dev/null 2>&1 &
PID_CM=$!
sleep 1
rodar comcm
if grep -q '^32$' "$OUT/comcm.prof"; then ok "com compositor: janela em 32 bits"; else nok "com compositor: janela em 32 bits"; fi
if [ "$(pixel "$OUT/comcm.png" 150 120)" = "20 60 110" ]; then
  ok "metade semitransparente: 50% sobre o fundo (20 60 110)"
else
  nok "metade semitransparente: 50% sobre o fundo (obtido $(pixel "$OUT/comcm.png" 150 120))"
fi

echo
echo "resultado: $PASS ok, $FAIL falha(s)"
[ "$FAIL" -eq 0 ]
