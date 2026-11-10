#!/bin/bash
# Testes de conformidade do kit X11 (kit::x11_layer) contra o padrão X11/ICCCM/EWMH.
# O programa cria as camadas; este script confere o resultado de fora, com xprop e
# xwininfo (ferramentas do X, independentes do kit). Só roda num display de teste:
# recusa :0 (sessão real) e :99 (de outra instância).
#
# Uso: tests/x11/conformidade.sh [:65]
set -u
DISP="${1:-:65}"
case "$DISP" in
  :0|:0.*|:99|:99.*) echo "recusado: $DISP não é display de teste"; exit 2 ;;
esac
DIR="$(cd "$(dirname "$0")/../.." && pwd)"
EX="$DIR/target/debug/examples/x11-conformidade"
OUT="$(mktemp -d)"
PASS=0; FAIL=0
ok()   { echo "  ok    $1"; PASS=$((PASS+1)); }
nok()  { echo "  FALHA $1"; FAIL=$((FAIL+1)); }
check() { # check "nome" comando...
  local nome="$1"; shift
  if "$@" >/dev/null 2>&1; then ok "$nome"; else nok "$nome"; fi
}

cleanup() {
  [ -n "${PID_EX:-}" ] && kill "$PID_EX" 2>/dev/null
  [ -n "${PID_XV:-}" ] && kill "$PID_XV" 2>/dev/null
  rm -rf "$OUT"
}
trap cleanup EXIT

if [ ! -x "$EX" ]; then
  echo "falta $EX (rode: cargo build --example x11-conformidade -j 2)"; exit 2
fi
if DISPLAY="$DISP" xdpyinfo >/dev/null 2>&1; then
  echo "recusado: $DISP já está em uso"; exit 2
fi

Xvfb "$DISP" -screen 0 1366x768x24 -nolisten tcp >/dev/null 2>&1 &
PID_XV=$!
sleep 2
export DISPLAY="$DISP"
unset WAYLAND_DISPLAY

echo "== servidor X ($DISP)"
check "extensão XTEST presente" bash -c "xdpyinfo | grep -q XTEST"
check "extensão SHAPE presente" bash -c "xdpyinfo | grep -q SHAPE"

"$EX" > "$OUT/saida.txt" 2>"$OUT/erro.txt" &
PID_EX=$!

esperar() { # esperar MARCADOR
  for _ in $(seq 1 100); do
    grep -q "^PASSO $1 " "$OUT/saida.txt" 2>/dev/null && return 0
    sleep 0.1
  done
  return 1
}

echo "== janelas criadas"
if esperar pronto; then
  ok "programa criou as três camadas"
else
  nok "programa criou as três camadas ($(cat "$OUT/erro.txt"))"; exit 1
fi
IDS=$(grep '^IDS ' "$OUT/saida.txt")
DOCK=$(echo "$IDS" | sed -n 's/.*dock=\([0-9]*\).*/\1/p')
NOTIF=$(echo "$IDS" | sed -n 's/.*notif=\([0-9]*\).*/\1/p')
POPUP=$(echo "$IDS" | sed -n 's/.*popup=\([0-9]*\).*/\1/p')
DESK=$(echo "$IDS" | sed -n 's/.*desktop=\([0-9]*\).*/\1/p')
hex() { printf '0x%x' "$1"; }
D=$(hex "$DOCK"); N=$(hex "$NOTIF"); P=$(hex "$POPUP")

# ICCCM 4.1.2.5: WM_CLASS é "instância\0classe\0".
check "WM_CLASS da dock (ICCCM 4.1.2.5)" bash -c "xprop -id $D WM_CLASS | grep -q '\"conf-dock\", \"conf-dock\"'"
check "_NET_WM_NAME em UTF8 (EWMH)" bash -c "xprop -id $D _NET_WM_NAME | grep -q 'conf-dock'"
check "tipo da dock é _NET_WM_WINDOW_TYPE_DOCK (EWMH)" bash -c "xprop -id $D _NET_WM_WINDOW_TYPE | grep -q _NET_WM_WINDOW_TYPE_DOCK"
check "tipo da notificação é _NET_WM_WINDOW_TYPE_NOTIFICATION (EWMH)" bash -c "xprop -id $N _NET_WM_WINDOW_TYPE | grep -q _NET_WM_WINDOW_TYPE_NOTIFICATION"
# _NET_WM_STRUT_PARTIAL: esquerda, direita, topo, base, e os intervalos (12 valores).
check "strut da dock: base 68, intervalo 0..1365 (EWMH)" bash -c "xprop -id $D _NET_WM_STRUT_PARTIAL | grep -q '0, 0, 0, 68, 0, 0, 0, 0, 0, 0, 0, 1365'"
check "dock sem override-redirect (o gerenciador vê a reserva)" bash -c "xwininfo -id $D | grep -q 'Override Redirect State: no'"
check "notificação com override-redirect (fora do gerenciador)" bash -c "xwininfo -id $N | grep -q 'Override Redirect State: yes'"
check "janela em profundidade 24 sem MIDE_X11_ARGB" bash -c "xwininfo -id $D | grep -q 'Depth: 24'"
check "dock mapeada (IsViewable)" bash -c "xwininfo -id $D | grep -q 'Map State: IsViewable'"
# Área de trabalho (EWMH _NET_WM_WINDOW_TYPE_DESKTOP): tela inteira e por baixo de tudo.
DK=$(hex "$DESK")
check "tipo da área de trabalho é _NET_WM_WINDOW_TYPE_DESKTOP (EWMH)" bash -c "xprop -id $DK _NET_WM_WINDOW_TYPE | grep -q _NET_WM_WINDOW_TYPE_DESKTOP"
check "área de trabalho cobre a tela (1366x768 em 0,0)" bash -c "xwininfo -id $DK | grep -q 'Width: 1366' && xwininfo -id $DK | grep -q 'Height: 768' && xwininfo -id $DK | grep -q 'Absolute upper-left X:  0'"
ORDEM=$(xwininfo -root -children | awk '{print $1}')
POS_DESK=$(echo "$ORDEM" | grep -n "^$DK$" | cut -d: -f1)
POS_DOCK=$(echo "$ORDEM" | grep -n "^$D$" | cut -d: -f1)
# xwininfo lista do topo para o fundo: a área de trabalho por baixo vem depois da dock.
check "área de trabalho por baixo da dock na pilha" test "$POS_DESK" -gt "$POS_DOCK"

echo "== mover"
if esperar movido; then
  check "geometria do popup depois de set_rect (ICCCM 4.1.5)" bash -c "xwininfo -id $P | grep -q 'Absolute upper-left X:  150' && xwininfo -id $P | grep -q 'Absolute upper-left Y:  120' && xwininfo -id $P | grep -q 'Width: 220' && xwininfo -id $P | grep -q 'Height: 60'"
else
  nok "popup moveu (marcador não chegou)"
fi

echo "== forma"
if esperar forma; then
  # xwininfo não mostra a área de ponteiro (SHAPE input), então o teste é de
  # comportamento: o programa imprime os cliques que a notificação recebe durante 3 s.
  # Clique dentro da área (1050,40) deve chegar (local 50,20); clique na mesma janela,
  # mas fora da área (1200,60), não deve chegar.
  sleep 0.5
  xdotool mousemove 1050 40 click 1
  sleep 0.3
  xdotool mousemove 1200 60 click 1
  sleep 0.3
  xdotool mousemove 600 600
else
  nok "forma (marcador não chegou)"
fi

echo "== destruir"
if esperar destruida; then
  check "popup removido da árvore depois de destroy" bash -c "! xwininfo -root -children | grep -q '$P'"
else
  nok "destruir (marcador não chegou)"
fi

wait "$PID_EX" 2>/dev/null
RC=$?
CLIQUES_NOTIF=$(grep -c "^CLIQUE $NOTIF " "$OUT/saida.txt")
check "um só clique chega à notificação (o de dentro da área)" test "$CLIQUES_NOTIF" -eq 1
check "o clique que chegou está em (50,20), na área de ponteiro" grep -q "^CLIQUE $NOTIF 50 20" "$OUT/saida.txt"
check "programa terminou sem erro (rc=$RC)" test "$RC" -eq 0

echo
echo "resultado: $PASS ok, $FAIL falha(s)"
[ "$FAIL" -eq 0 ]
