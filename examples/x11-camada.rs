//! Teste das camadas X11 (`kit::x11_layer`): cria uma barra de 320x40 no canto, envia um
//! quadro, lê os eventos por 6 s e digita "a" com XTEST. Rodar com um `DISPLAY` X11 de
//! teste (ex.: `DISPLAY=:65 cargo run --example x11-camada`), com `xev` ou outro app com foco.

use std::time::{Duration, Instant};

use mide_ui::kit::x11_layer::{Edge, Input, Kind, LayerSpec, Rect, Session};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect as SkRect, Transform};

fn main() {
    let session = match Session::connect() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sem X11: {e}");
            return;
        }
    };
    let (sw, sh) = session.screen_size();
    println!("tela {sw}x{sh}, XTEST={}", session.has_xtest());
    let area = session.work_area();
    println!("área útil: {area:?}");

    let rect = Rect { x: 100, y: 100, w: 320, h: 40 };
    let spec = LayerSpec { app_id: "mide-camada-teste".into(), kind: Kind::Dock, rect, strut: Some((Edge::Top, 40)), keyboard: false };
    let mut layer = match session.layer(&spec) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("camada: {e}");
            return;
        }
    };

    let mut canvas = Pixmap::new(rect.w, rect.h).expect("quadro");
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(40, 120, 220, 255));
    let mut pb = PathBuilder::new();
    pb.push_rect(SkRect::from_xywh(0.0, 0.0, rect.w as f32, rect.h as f32).unwrap());
    canvas.fill_path(&pb.finish().unwrap(), &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
    if let Err(e) = layer.present(&canvas, [0, 0, 0]) {
        eprintln!("quadro: {e}");
    }

    let end = Instant::now() + Duration::from_secs(6);
    let mut typed = false;
    let mut counts = (0, 0, 0);
    while Instant::now() < end {
        match session.poll() {
            Ok(inputs) => {
                for input in inputs {
                    match input {
                        Input::Pointer { .. } => counts.0 += 1,
                        Input::Expose { .. } => counts.1 += 1,
                        Input::Key { .. } => counts.2 += 1,
                        Input::Closed => println!("conexão fechada"),
                    }
                }
            }
            Err(e) => {
                eprintln!("eventos: {e}");
                break;
            }
        }
        if !typed && Instant::now() + Duration::from_secs(3) < end {
            typed = true;
            match session.type_keysym('a' as u32, false) {
                Ok(true) => println!("digitou 'a' por XTEST"),
                Ok(false) => println!("'a' não está no layout"),
                Err(e) => println!("XTEST: {e}"),
            }
            // Um caractere fora do layout (á, U+00E1 = keysym 0x1e1).
            match session.type_keysym_remapped(0x1e1) {
                Ok(true) => println!("digitou 'á' pelo remapeamento"),
                Ok(false) => println!("sem tecla livre"),
                Err(e) => println!("remapeamento: {e}"),
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    println!("eventos: ponteiro={} expose={} tecla={}", counts.0, counts.1, counts.2);

    // Forma e área de ponteiro: só a metade esquerda recebe o ponteiro.
    if let Err(e) = layer.set_input_region(Some(Rect { x: 0, y: 0, w: 160, h: 40 })) {
        println!("área de ponteiro: {e}");
    }
    if let Err(e) = layer.set_rect(Rect { x: 120, y: 120, w: 320, h: 40 }) {
        println!("mover: {e}");
    }
    println!("fim");
}
