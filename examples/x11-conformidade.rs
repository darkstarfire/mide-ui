//! Programa dos testes de conformidade X11 (`tests/x11/conformidade.sh`). Cria três
//! camadas, imprime os ids e marcadores de passo, e executa as mudanças que o script
//! confere de fora (com `xprop` e `xwininfo`, que não usam o kit): mover, trocar a área
//! de ponteiro e destruir. Os marcadores têm o formato `PASSO <nome> <id>`.

use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

use mide_ui::kit::x11_layer::{Edge, Input, Kind, LayerSpec, PointerKind, Rect, Session};

fn marca(passo: &str, id: u32) {
    println!("PASSO {passo} {id}");
    let _ = std::io::stdout().flush();
}

fn main() {
    let session = match Session::connect() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sem X11: {e}");
            std::process::exit(2);
        }
    };

    // Área de trabalho em tela inteira: tem que ficar por baixo das outras camadas.
    let desktop = session
        .layer(&LayerSpec {
            app_id: "conf-desktop".into(),
            kind: Kind::Desktop,
            rect: Rect { x: 0, y: 0, w: 1366, h: 768 },
            strut: None,
            keyboard: false,
        })
        .expect("área de trabalho");

    // Dock com borda reservada embaixo (68 px, largura da tela).
    let dock = session
        .layer(&LayerSpec {
            app_id: "conf-dock".into(),
            kind: Kind::Dock,
            rect: Rect { x: 0, y: 700, w: 1366, h: 68 },
            strut: Some((Edge::Bottom, 68)),
            keyboard: false,
        })
        .expect("dock");
    // Notificação sem borda reservada.
    let mut notif = session
        .layer(&LayerSpec {
            app_id: "conf-notif".into(),
            kind: Kind::Notification,
            rect: Rect { x: 1000, y: 20, w: 300, h: 80 },
            strut: None,
            keyboard: false,
        })
        .expect("notificação");
    // Popup que será movido e destruído.
    let mut popup = session
        .layer(&LayerSpec {
            app_id: "conf-popup".into(),
            kind: Kind::Dock,
            rect: Rect { x: 100, y: 100, w: 200, h: 50 },
            strut: None,
            keyboard: false,
        })
        .expect("popup");

    marca("pronto", 0);
    println!("IDS dock={} notif={} popup={} desktop={}", dock.window(), notif.window(), popup.window(), desktop.window());
    let _ = std::io::stdout().flush();
    sleep(Duration::from_secs(3));

    // Mover o popup: a geometria tem que refletir o pedido.
    popup
        .set_rect(Rect { x: 150, y: 120, w: 220, h: 60 })
        .expect("mover");
    marca("movido", popup.window());
    sleep(Duration::from_secs(2));

    // Área de ponteiro da notificação: só o canto de cima à esquerda.
    notif
        .set_input_region(Some(Rect { x: 0, y: 0, w: 100, h: 40 }))
        .expect("área de ponteiro");
    marca("forma", notif.window());
    // Durante 3 s, lê os eventos da conexão e imprime os cliques que a notificação
    // recebe (o script conta: só o de dentro da área de ponteiro deve chegar).
    let fim = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < fim {
        for input in session.poll().expect("eventos") {
            if let Input::Pointer { window, x, y, kind: PointerKind::Press(1) } = input {
                println!("CLIQUE {window} {x} {y}");
                let _ = std::io::stdout().flush();
            }
        }
        sleep(Duration::from_millis(20));
    }

    // Destruir o popup: a janela tem que sumir da árvore.
    let id = popup.window();
    popup.destroy().expect("destruir");
    marca("destruida", id);
    sleep(Duration::from_secs(2));

    marca("fim", 0);
    drop(dock);
    drop(notif);
    drop(desktop);
}
