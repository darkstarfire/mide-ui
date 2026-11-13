//! Laço de um app do kit numa janela X11 (`kit::run` sem Wayland). É a versão mínima do
//! laço do Wayland: uma camada com o quadro do app, o ponteiro (cliques, rolagem, sair
//! e entrar), o texto digitado e as teclas especiais. Os menus da barra superior e as
//! janelas filhas não existem aqui (o X11 não tem o compositor do mide para isso).

use super::ui::WindowRequest;
use std::error::Error;
use std::os::fd::BorrowedFd;
use std::time::{Duration, Instant};

use rustix::event::{poll, PollFd, PollFlags};
use tiny_skia::Pixmap;

use super::app::{App, Window};
use super::input::Key;
use super::ui::{Ctx, Ui};
use super::x11_layer::{Edge, Input, Kind, Layer, LayerSpec, PointerKind, Rect, Session};

/// Rolagem de uma volta da roda, em pixels (como o Wayland, positivo = para baixo).
const WHEEL_STEP: f32 = 40.0;
/// Sem nada acontecendo, o app é desenhado de novo a cada tanto (animações e vidro).
const IDLE_TICK: Duration = Duration::from_millis(100);
/// Com o botão apertado, o toque longo e as animações pedem quadros mais seguidos.
const PRESSED_TICK: Duration = Duration::from_millis(16);
/// Com o bloqueio ativo, a janela volta ao topo da pilha de tempos em tempos.
const RAISE_EVERY: Duration = Duration::from_millis(500);

/// Roda `app` na janela `window` do X11.
pub fn run(window: Window, mut app: impl App) -> Result<(), Box<dyn Error>> {
    let session = Session::connect()?;
    let (sw, sh) = session.screen_size();
    let (w, h) = window.size;
    // Área de trabalho: tela inteira, no fundo da pilha, com o papel de parede por baixo.
    let base = if window.desktop { window.backdrop.and_then(|f| f(sw, sh)) } else { None };
    // Tela de bloqueio: a tela inteira, por cima de tudo, com teclado e ponteiro presos.
    let rect = if window.desktop || window.lock {
        Rect { x: 0, y: 0, w: sw, h: sh }
    } else if window.corner {
        Rect { x: sw as i32 - w as i32, y: sh as i32 - h as i32, w, h }
    } else if let Some((Edge::Bottom, _)) = window.exclusive {
        // Reserva a borda de baixo: ocupa a largura inteira.
        Rect { x: 0, y: sh as i32 - h as i32, w: sw, h }
    } else {
        Rect { x: (sw as i32 - w as i32) / 2, y: (sh as i32 - h as i32) / 2, w, h }
    };
    let spec = LayerSpec {
        app_id: window.app_id.clone(),
        kind: if window.desktop {
            Kind::Desktop
        } else if window.full_size && !window.lock && !window.corner && window.exclusive.is_none() && window.overlay.is_none() {
            // Conteúdo inteiro: janela comum, adotada pelo gerenciador (ver `Kind::ContentWindow`).
            Kind::ContentWindow
        } else {
            Kind::Dock
        },
        rect,
        strut: if window.desktop || window.lock { None } else { window.exclusive },
        keyboard: window.lock,
    };
    let mut layer = session.layer(&spec)?;
    tracing::info!("janela X11 {} pronta ({}x{})", window.app_id, rect.w, rect.h);
    if window.lock {
        layer.raise();
        if !layer.grab_keyboard() || !layer.grab_pointer() {
            tracing::warn!("bloqueio X11: o servidor recusou prender o teclado ou o ponteiro");
        }
    }
    let mut last_raise = Instant::now();
    // Janela de conteúdo inteiro: cantos arredondados pelo próprio recorte (sem compositor),
    // refeitos quando o tamanho muda (tela cheia tira o recorte).
    let content_window = window.full_size && !window.lock && !window.corner && window.exclusive.is_none() && window.overlay.is_none() && !window.desktop;
    let mut shaped: Option<(u32, u32)> = None;

    let mut ctx = Ctx::new(window.theme.clone());
    // Vidro só com alfa real: sem ARGB, o fundo translúcido sairia cinza.
    ctx.glass_allowed = session.has_argb();
    ctx.no_alpha = !session.has_argb();
    ctx.content_window = window.full_size;
    // Overlay sem alfa que não cobre a tela (mide-search): sem fundo da cor do tema, que
    // sairia como retângulo; a forma da janela segue o que foi desenhado.
    let shape_overlay = window.overlay.is_some() && !session.has_argb() && (rect.w < sw || rect.h < sh);
    ctx.opaque_bg = !window.desktop && !window.lock && !shape_overlay;
    let mut last_mask: Option<Vec<Rect>> = None;
    let mut pressed = false;
    let mut needs_frame = true;
    let mut last_frame = Instant::now();

    loop {
        // Espera o servidor (ou o tempo de um quadro). O descritor da conexão é do X.
        let timeout = if pressed { PRESSED_TICK } else { IDLE_TICK };
        let fd = session.fd();
        // SAFETY: a conexão X (dona do descritor) vive enquanto o laço roda.
        let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
        let mut fds = [PollFd::new(&borrowed, PollFlags::IN)];
        let ts = rustix::fs::Timespec { tv_sec: timeout.as_secs() as _, tv_nsec: timeout.subsec_nanos() as _ };
        let _ = poll(&mut fds, Some(&ts));

        let events = session.poll()?;
        let mut frame_input = FrameInput::default();
        for event in events {
            match event {
                Input::Pointer { window: win, x, y, kind } if win == layer.window() => {
                    // Clique numa janela de conteúdo leva o foco de teclado para ela.
                    if content_window && matches!(kind, PointerKind::Press(_)) {
                        layer.focus();
                    }
                    frame_input.pointer(x, y, kind, &mut pressed);
                }
                Input::Pointer { .. } => {}
                Input::Expose { window: win } if win == layer.window() => needs_frame = true,
                Input::Expose { .. } => {}
                Input::Key { window: win, keysym, pressed: down, shift } if win == layer.window() => {
                    if down {
                        frame_input.key(keysym);
                        frame_input.shift = Some(shift);
                    }
                }
                Input::Key { .. } => {}
                Input::Closed => return Ok(()),
            }
        }
        let any = frame_input.any();
        frame_input.apply(&mut ctx.input);
        if any {
            needs_frame = true;
        }

        // Bloqueio: outras janelas que aparecem não podem ficar por cima dele.
        if window.lock && last_raise.elapsed() >= RAISE_EVERY {
            layer.raise();
            last_raise = Instant::now();
        }

        if needs_frame || pressed || last_frame.elapsed() >= IDLE_TICK {
            needs_frame = false;
            last_frame = Instant::now();
            let mask = draw(&mut app, &mut ctx, &mut layer, rect, base.as_ref(), shape_overlay)?;
            // Forma nova só quando muda; se o servidor recusar, tenta de novo no quadro seguinte.
            if let Some(rects) = mask.filter(|r| last_mask.as_ref() != Some(r)) {
                last_mask = layer.set_bounding_rects(&rects).ok().map(|()| rects);
            }
            if content_window {
                if let Ok((cw, ch)) = layer.size() {
                    if shaped != Some((cw, ch)) {
                        let full = cw == sw && ch == sh;
                        let _ = layer.round_corners(cw, ch, if full { 0.0 } else { 16.0 });
                        shaped = Some((cw, ch));
                    }
                }
            }
            // Luzes da janela de conteúdo inteiro: o pedido vai ao gerenciador (que faz a ação).
            if let Some(request) = ctx.window_request.take() {
                tracing::debug!("janela {}: pedido {request:?} enviado ao gerenciador", window.app_id);
                let sent = match request {
                    WindowRequest::Close => layer.wm_close(),
                    WindowRequest::Minimize => layer.wm_minimize(),
                    WindowRequest::ToggleFullscreen => layer.wm_toggle_fullscreen(),
                    WindowRequest::ToggleMaximize => layer.wm_toggle_maximize(),
                    WindowRequest::Move => layer.wm_move(),
                };
                if let Err(err) = sent {
                    tracing::warn!("pedido à janela não foi enviado ({err})");
                }
            }
            // Senha aceita (ou o app pediu para sair): desbloqueia e termina.
            if std::mem::take(&mut ctx.unlock_request) {
                break;
            }
            if std::mem::take(&mut ctx.close_requested) && app.close_requested() {
                break;
            }
            if ctx.repaint {
                needs_frame = true;
            }
        }
    }
    if window.lock {
        layer.ungrab_keyboard();
        layer.ungrab_pointer();
    }
    drop(layer);
    Ok(())
}

/// Um quadro: a interface do app (`view`) desenhada num `Pixmap` e enviada à camada.
/// `mask`: devolve a forma do quadro (alfa >= 128) para a janela recortar com ela.
fn draw(app: &mut impl App, ctx: &mut Ctx, layer: &mut Layer, rect: Rect, base: Option<&Pixmap>, mask: bool) -> Result<Option<Vec<Rect>>, Box<dyn Error>> {
    crate::ui::draw::set_scale(1.0);
    // O fundo (papel de parede) fica por baixo; sem ele, o quadro começa transparente.
    let mut canvas = match base {
        Some(b) => b.clone(),
        None => Pixmap::new(rect.w.max(1), rect.h.max(1)).ok_or("quadro de tamanho zero")?,
    };
    ctx.begin_frame();
    {
        let mut ui = Ui::root(&mut canvas, ctx);
        app.view(&mut ui);
    }
    ctx.paint_overlay(&mut canvas);
    ctx.end_frame();
    // Os eventos de um quadro valem só para ele.
    let input = &mut ctx.input;
    input.pressed = false;
    input.released = false;
    input.secondary = false;
    input.scroll = 0.0;
    input.text.clear();
    input.keys.clear();
    // Sem alfa (X11 sem ARGB), o que é translúcido se mistura com a cor de fundo enviada:
    // numa janela de conteúdo, a cor do tema (senão sai preto); nas camadas, o preto.
    let background = if ctx.opaque_bg {
        let c = ctx.theme.background;
        [c[0], c[1], c[2]]
    } else {
        [0, 0, 0]
    };
    layer.present(&canvas, background).map_err(|e| e.to_string())?;
    Ok(mask.then(|| super::x11_layer::alpha_rects(&canvas, 128)))
}

/// Eventos que chegaram desde o último quadro, já com as mudanças do ponteiro.
#[derive(Default)]
struct FrameInput {
    /// Shift da última tecla do quadro (`None` se não houve tecla).
    shift: Option<bool>,
    pointer: Option<(f32, f32)>,
    pointer_gone: bool,
    pressed: bool,
    released: bool,
    down: Option<bool>,
    secondary: bool,
    scroll: f32,
    text: String,
    keys: Vec<Key>,
}

impl FrameInput {
    fn pointer(&mut self, x: f64, y: f64, kind: PointerKind, pressed: &mut bool) {
        let at = Some((x as f32, y as f32));
        match kind {
            PointerKind::Motion | PointerKind::Enter => self.pointer = at,
            PointerKind::Leave => self.pointer_gone = true,
            PointerKind::Press(1) => {
                self.pointer = at;
                self.pressed = true;
                self.down = Some(true);
                *pressed = true;
            }
            PointerKind::Release(1) => {
                self.pointer = at;
                self.released = true;
                self.down = Some(false);
                *pressed = false;
            }
            PointerKind::Press(3) => {
                self.pointer = at;
                self.secondary = true;
            }
            // A roda chega como botões 4 (para cima) e 5 (para baixo).
            PointerKind::Press(4) => self.scroll -= WHEEL_STEP,
            PointerKind::Press(5) => self.scroll += WHEEL_STEP,
            _ => {}
        }
    }

    /// Tecla especial ou caractere (o keysym do X). Sem modificadores: o X manda o
    /// keysym da coluna sem Shift, então letras maiúsculas chegam como minúsculas.
    fn key(&mut self, keysym: u32) {
        let special = match keysym {
            0xff0d | 0xff8d => Some(Key::Enter),
            0xff1b => Some(Key::Escape),
            0xff08 => Some(Key::Backspace),
            0xff09 => Some(Key::Tab),
            0xff51 => Some(Key::Left),
            0xff52 => Some(Key::Up),
            0xff53 => Some(Key::Right),
            0xff54 => Some(Key::Down),
            0xffff => Some(Key::Delete),
            0xff50 => Some(Key::Home),
            0xff57 => Some(Key::End),
            0xff55 => Some(Key::PageUp),
            0xff56 => Some(Key::PageDown),
            _ => None,
        };
        if let Some(key) = special {
            self.keys.push(key);
            return;
        }
        if let Some(c) = keysym_char(keysym) {
            self.text.push(c);
        }
    }

    fn any(&self) -> bool {
        self.pointer.is_some() || self.pointer_gone || self.pressed || self.released || self.down.is_some() || self.secondary || self.scroll != 0.0 || !self.text.is_empty() || !self.keys.is_empty()
    }

    fn apply(self, input: &mut super::input::Input) {
        if let Some(shift) = self.shift {
            input.shift = shift;
        }
        if self.pointer_gone {
            input.pointer = None;
        }
        if let Some(p) = self.pointer {
            input.pointer = Some(p);
        }
        if self.pressed {
            input.pressed = true;
        }
        if self.released {
            input.released = true;
        }
        if let Some(down) = self.down {
            input.down = down;
        }
        if self.secondary {
            input.secondary = true;
        }
        input.scroll += self.scroll;
        input.text.push_str(&self.text);
        input.keys.extend(self.keys);
    }
}

/// Caractere de um keysym do X (latin-1 e Unicode); `None` para teclas sem texto.
fn keysym_char(keysym: u32) -> Option<char> {
    match keysym {
        0x20..=0x7e | 0xa0..=0xff => char::from_u32(keysym),
        0x0100_0000..=0x0110_ffff => char::from_u32(keysym - 0x0100_0000),
        _ => None,
    }
}
