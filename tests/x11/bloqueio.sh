#!/bin/bash
# Testes da tela de bloqueio no X11 (Window::lock no kit::x11_app), no display de teste.
# O examples/x11-bloqueio abre a tela de bloqueio; o script confere de fora: tela inteira,
# por cima de outra janela que aparece depois, teclas com Shift chegando ao app, e saída
# ao desbloquear. Recusa :0 e :99.
#
# Uso: tests/x11/bloqueio.sh [:65]
set -u
DISP="${1:-:65}"
case "$DISP" in
  :0|:0.*|:99|:99.*) echo "recusado: $DISP não é display de teste"; exit 2 ;;
esac
DIR="$(cd "$(dirname "$0")/../.." && pwd)"
EX="$DIR/target/debug/examples/x11-bloqueio"
OUT="$(mktemp -d)"
PASS=0; FAIL=0
ok()  { echo "  ok    $1"; PASS=$((PASS+1)); }
nok() { echo "  FALHA $1"; FAIL=$((FAIL+1)); }
check() { local nome="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$nome"; else nok "$nome"; fi; }
cleanup() {
  [ -n "${PID_EX:-}" ] && kill "$PID_EX" 2>/dev/null
  [ -n "${PID_XT:-}" ] && kill "$PID_XT" 2>/dev/null
  [ -n "${PID_XV:-}" ] && kill "$PID_XV" 2>/dev/null
  rm -rf "$OUT"
}
trap cleanup EXIT

[ -x "$EX" ] || { echo "falta $EX (cargo build --example x11-bloqueio -j 2)"; exit 2; }
if DISPLAY="$DISP" xdpyinfo >/dev/null 2>&1; then echo "recusado: $DISP já está em uso"; exit 2; fi

Xvfb "$DISP" -screen 0 1366x768x24 -nolisten tcp >/dev/null 2>&1 &
PID_XV=$!
sleep 2
export DISPLAY="$DISP"
unset WAYLAND_DISPLAY

"$EX" > "$OUT/saida.txt" 2>"$OUT/erro.txt" &
PID_EX=$!
esperar() { for _ in $(seq 1 150); do grep -q "$1" "$OUT/saida.txt" 2>/dev/null && return 0; sleep 0.1; done; return 1; }

echo "== tela de bloqueio"
if esperar PRONTO; then ok "programa de bloqueio iniciou"; else nok "programa de bloqueio iniciou ($(cat "$OUT/erro.txt"))"; exit 1; fi
sleep 1
ID=$(xwininfo -root -children | grep 'conf-bloqueio"' | awk '{print $1}' | head -1)
[ -n "$ID" ] || { nok "janela do bloqueio apareceu"; exit 1; }
ok "janela do bloqueio apareceu"
check "tela inteira (1366x768 em 0,0)" bash -c "xwininfo -id $ID | grep -q 'Width: 1366' && xwininfo -id $ID | grep -q 'Height: 768' && xwininfo -id $ID | grep -q 'Absolute upper-left X:  0'"
check "fora do gerenciador (override-redirect), por cima de tudo" bash -c "xwininfo -id $ID | grep -q 'Override Redirect State: yes'"

echo "== por cima de outra janela"
xterm -geometry 60x10+100+100 -e sleep 60 >/dev/null 2>&1 &
PID_XT=$!
sleep 2
ORDEM=$(xwininfo -root -children | awk '{print $1}')
POS_LOCK=$(echo "$ORDEM" | grep -n "^$ID$" | cut -d: -f1)
XT=$(xdotool search --class xterm | head -1)
XTW=$(xwininfo -root -children | grep -i 'xterm\|"sleep"' | awk '{print $1}' | head -1)
POS_XT=$(echo "$ORDEM" | grep -n "^$XTW$" | cut -d: -f1)
# xwininfo lista do topo para o fundo: o bloqueio tem que vir antes do xterm.
check "bloqueio acima da janela que apareceu depois" test -n "$POS_XT" -a "$POS_LOCK" -lt "$POS_XT"

echo "== teclas"
sleep 0.5
xdotool key shift+a
sleep 0.4
xdotool key q
for _ in $(seq 1 60); do grep -q DESBLOQUEIO "$OUT/saida.txt" 2>/dev/null && break; sleep 0.1; done
check "tecla com Shift chega como maiúscula (shift=true)" grep -q 'TECLA "A" shift=true' "$OUT/saida.txt"

echo "== desbloquear"
wait "$PID_EX" 2>/dev/null
RC=$?
check "o 'q' desbloqueou e o programa saiu (sem erro)" bash -c "grep -q DESBLOQUEIO '$OUT/saida.txt' && test $RC -eq 0"
check "a janela do bloqueio foi destruída" bash -c "! xwininfo -root -children | grep -q 'conf-bloqueio\"'"
PID_EX=""

echo
echo "resultado: $PASS ok, $FAIL falha(s)"
[ "$FAIL" -eq 0 ]
