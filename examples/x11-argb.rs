//! Teste das camadas com alfa (`MIDE_X11_ARGB=1`): uma faixa de 320x40 em que a metade
//! esquerda é azul com 50% de alfa e a direita é totalmente transparente. Usado por
//! `tests/x11/argb.sh`, que confere a profundidade da janela e a mistura dos pixels.

use std::time::{Duration, Instant};

use mide_ui::kit::x11_layer::{Kind, LayerSpec, Rect, Session};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect as SkRect, Transform};

fn main() {
    let session = match Session::connect() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sem X11: {e}");
            std::process::exit(2);
        }
    };
    let rect = Rect { x: 100, y: 100, w: 320, h: 40 };
    let spec = LayerSpec { app_id: "mide-argb-teste".into(), kind: Kind::Dock, rect, strut: None, keyboard: false };
    let mut layer = match session.layer(&spec) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("camada: {e}");
            std::process::exit(2);
        }
    };
    let mut canvas = Pixmap::new(rect.w, rect.h).expect("quadro");
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(40, 120, 220, 128));
    let mut pb = PathBuilder::new();
    pb.push_rect(SkRect::from_xywh(0.0, 0.0, 160.0, 40.0).unwrap());
    canvas.fill_path(&pb.finish().unwrap(), &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
    if let Err(e) = layer.present(&canvas, [0, 0, 0]) {
        eprintln!("quadro: {e}");
        std::process::exit(2);
    }
    println!("PRONTO");
    let end = Instant::now() + Duration::from_secs(8);
    while Instant::now() < end {
        let _ = session.poll();
        std::thread::sleep(Duration::from_millis(50));
    }
}
