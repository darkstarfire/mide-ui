//! Camadas do X11: janelas que se comportam como as camadas do Wayland (barra, dock,
//! notificações, menus), sem o compositor do mide. Serve os programas da shell quando o
//! mide roda como X11 (`MIDE_BACKEND=x11`, ou um X11 de verdade por baixo, como o KWin).
//!
//! - Cada camada é uma janela do tamanho do que ela desenha (`Rect`, em coordenadas de
//!   tela). O quadro é um `Pixmap` do tamanho da janela; `Layer::present` o envia.
//! - Com `MIDE_X11_ARGB=1` e um visual de 32 bits (TrueColor com alfa), as partes
//!   transparentes do quadro ficam transparentes na janela (precisa de um compositor,
//!   como o KWin ou o picom). Sem isso, o quadro é composto sobre a cor de fundo.
//! - Dock e barra usam a dica `_NET_WM_STRUT_PARTIAL` (a área que elas reservam).
//! - A forma (`set_bounding_region`) e a área de ponteiro (`set_input_region`) usam a
//!   extensão SHAPE. A digitação de teclas na tela usa a extensão XTEST.
//!
//! Os eventos chegam por `Session::poll` (quando `Session::fd` acorda). Ponteiro e
//! teclas vêm com a janela de origem; as coordenadas do ponteiro são da janela.

use std::fmt;
use std::os::fd::AsRawFd;
use std::sync::Arc;

use tiny_skia::Pixmap;
use x11rb::connection::Connection;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{self, ConnectionExt as _, Window};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME, NONE};

/// Erro do X11 (texto para o usuário e o log).
#[derive(Debug, Clone)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

fn err<E: fmt::Display>(e: E) -> Error {
    Error(e.to_string())
}

/// Retângulo em coordenadas de tela, em pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Borda da tela onde a camada reserva espaço (`strut`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Tipo da camada: a dica `_NET_WM_WINDOW_TYPE` que o gerenciador de janelas lê.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dock,
    Notification,
    /// Área de trabalho: fica no fundo da pilha (`_NET_WM_WINDOW_TYPE_DESKTOP`).
    Desktop,
}

/// O que criar: nome do app (`WM_CLASS`), tipo, posição e tamanho, reserva de borda e
/// se a camada recebe o foco do teclado.
#[derive(Debug, Clone)]
pub struct LayerSpec {
    pub app_id: String,
    pub kind: Kind,
    pub rect: Rect,
    pub strut: Option<(Edge, u32)>,
    pub keyboard: bool,
}

/// Tipo de evento de ponteiro. Os botões são os do X (1 esquerdo, 2 meio, 3 direito;
/// 4 a 7 são a rolagem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Motion,
    Enter,
    Leave,
    Press(u8),
    Release(u8),
}

/// Evento de uma camada. `Pointer` traz as coordenadas relativas à janela `window`.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Pointer { window: u32, x: f64, y: f64, kind: PointerKind },
    Expose { window: u32 },
    Key { window: u32, keysym: u32, pressed: bool },
    /// A conexão com o servidor acabou.
    Closed,
}

struct Atoms {
    net_wm_window_type: xproto::Atom,
    net_wm_window_type_dock: xproto::Atom,
    net_wm_window_type_notification: xproto::Atom,
    net_wm_window_type_desktop: xproto::Atom,
    net_wm_strut_partial: xproto::Atom,
    net_wm_strut: xproto::Atom,
    net_wm_name: xproto::Atom,
    net_workarea: xproto::Atom,
    utf8_string: xproto::Atom,
    wm_hints: xproto::Atom,
}

struct Inner {
    conn: RustConnection,
    root: Window,
    root_visual: u32,
    root_depth: u8,
    /// Visual de 32 bits com alfa (TrueColor), se o servidor tem um.
    argb_visual: Option<u32>,
    /// Usa o visual com alfa (`MIDE_X11_ARGB=1` e o visual existe).
    argb: bool,
    width: u32,
    height: u32,
    xtest: bool,
    shape: bool,
    atoms: Atoms,
    min_keycode: u8,
    max_keycode: u8,
}

/// Conexão com o servidor X11 e as camadas criadas por ela. Clonar é barato (é uma
/// referência à mesma conexão).
#[derive(Clone)]
pub struct Session {
    inner: Arc<Inner>,
}

impl Session {
    /// Abre a conexão com o `DISPLAY` atual.
    pub fn connect() -> Result<Self> {
        let (conn, screen_num) = RustConnection::connect(None).map_err(err)?;
        let screen = conn.setup().roots.get(screen_num).ok_or_else(|| Error("tela X sem raiz".into()))?;
        let root = screen.root;
        let root_visual = screen.root_visual;
        let root_depth = screen.root_depth;
        let (width, height) = (screen.width_in_pixels as u32, screen.height_in_pixels as u32);

        // Procura um visual TrueColor de 32 bits (com alfa) para as camadas transparentes.
        let mut argb_visual = None;
        for depth in &screen.allowed_depths {
            if depth.depth != 32 {
                continue;
            }
            for visual in &depth.visuals {
                if visual.class == xproto::VisualClass::TRUE_COLOR {
                    argb_visual = Some(visual.visual_id);
                }
            }
        }
        let argb = argb_visual.is_some() && std::env::var("MIDE_X11_ARGB").ok().as_deref() == Some("1");

        let xtest = conn.query_extension(b"XTEST").map_err(err)?.reply().map_err(err)?.present;
        let shape = conn.query_extension(b"SHAPE").map_err(err)?.reply().map_err(err)?.present;

        let atoms = Atoms {
            net_wm_window_type: intern(&conn, "_NET_WM_WINDOW_TYPE")?,
            net_wm_window_type_dock: intern(&conn, "_NET_WM_WINDOW_TYPE_DOCK")?,
            net_wm_window_type_notification: intern(&conn, "_NET_WM_WINDOW_TYPE_NOTIFICATION")?,
            net_wm_window_type_desktop: intern(&conn, "_NET_WM_WINDOW_TYPE_DESKTOP")?,
            net_wm_strut_partial: intern(&conn, "_NET_WM_STRUT_PARTIAL")?,
            net_wm_strut: intern(&conn, "_NET_WM_STRUT")?,
            net_wm_name: intern(&conn, "_NET_WM_NAME")?,
            net_workarea: intern(&conn, "_NET_WORKAREA")?,
            utf8_string: intern(&conn, "UTF8_STRING")?,
            wm_hints: xproto::AtomEnum::WM_HINTS.into(),
        };
        let (min_keycode, max_keycode) = (conn.setup().min_keycode, conn.setup().max_keycode);

        Ok(Self {
            inner: Arc::new(Inner {
                conn,
                root,
                root_visual,
                root_depth,
                argb_visual,
                argb,
                width,
                height,
                xtest,
                shape,
                atoms,
                min_keycode,
                max_keycode,
            }),
        })
    }

    /// Tamanho da tela (raiz), em pixels.
    pub fn screen_size(&self) -> (u32, u32) {
        (self.inner.width, self.inner.height)
    }

    /// Área útil (sem as bordas reservadas pelos docks), a de `_NET_WORKAREA`; sem a
    /// dica, a tela inteira.
    pub fn work_area(&self) -> Rect {
        let i = &self.inner;
        let full = Rect { x: 0, y: 0, w: i.width, h: i.height };
        let reply = i.conn.get_property(false, i.root, i.atoms.net_workarea, xproto::AtomEnum::CARDINAL, 0, 4).ok().and_then(|c| c.reply().ok());
        match reply {
            Some(r) if r.value_len >= 4 => {
                let v: Vec<u32> = r.value32().map(|it| it.collect()).unwrap_or_default();
                if v.len() >= 4 && v[2] > 0 && v[3] > 0 {
                    return Rect { x: v[0] as i32, y: v[1] as i32, w: v[2], h: v[3] };
                }
                full
            }
            _ => full,
        }
    }

    /// Descritor da conexão, para o laço de eventos acordar quando chega algo.
    pub fn fd(&self) -> i32 {
        self.inner.conn.stream().as_raw_fd()
    }

    /// A extensão XTEST existe? (sem ela, as teclas da tela não chegam aos apps)
    pub fn has_xtest(&self) -> bool {
        self.inner.xtest
    }

    /// Cria uma camada (janela) com o `spec`.
    pub fn layer(&self, spec: &LayerSpec) -> Result<Layer> {
        let i = &self.inner;
        let conn = &i.conn;
        let override_redirect = spec.strut.is_none();
        let (depth, visual, colormap) = match (i.argb, i.argb_visual) {
            (true, Some(vis)) => {
                let cmap = conn.generate_id().map_err(err)?;
                conn.create_colormap(xproto::ColormapAlloc::NONE, cmap, i.root, vis).map_err(err)?;
                (32u8, vis, Some(cmap))
            }
            _ => (COPY_DEPTH_FROM_PARENT, i.root_visual, None),
        };
        let window = conn.generate_id().map_err(err)?;
        let mut aux = xproto::CreateWindowAux::new()
            .event_mask(
                xproto::EventMask::EXPOSURE
                    | xproto::EventMask::BUTTON_PRESS
                    | xproto::EventMask::BUTTON_RELEASE
                    | xproto::EventMask::POINTER_MOTION
                    | xproto::EventMask::ENTER_WINDOW
                    | xproto::EventMask::LEAVE_WINDOW
                    | xproto::EventMask::KEY_PRESS
                    | xproto::EventMask::KEY_RELEASE
                    | xproto::EventMask::STRUCTURE_NOTIFY,
            )
            .override_redirect(u32::from(override_redirect))
            .background_pixel(0)
            .border_pixel(0);
        if let Some(cmap) = colormap {
            aux = aux.colormap(cmap);
        }
        let r = spec.rect;
        conn.create_window(depth, window, i.root, r.x as i16, r.y as i16, r.w.max(1) as u16, r.h.max(1) as u16, 0, xproto::WindowClass::INPUT_OUTPUT, visual, &aux)
            .map_err(err)?;

        // Classe e nome (o gerenciador e a dock procuram pelo WM_CLASS).
        let class = format!("{0}\0{0}\0", spec.app_id);
        conn.change_property8(xproto::PropMode::REPLACE, window, xproto::AtomEnum::WM_CLASS, xproto::AtomEnum::STRING, class.as_bytes()).map_err(err)?;
        conn.change_property8(xproto::PropMode::REPLACE, window, i.atoms.net_wm_name, i.atoms.utf8_string, spec.app_id.as_bytes()).map_err(err)?;
        let wtype = match spec.kind {
            Kind::Dock => i.atoms.net_wm_window_type_dock,
            Kind::Notification => i.atoms.net_wm_window_type_notification,
            Kind::Desktop => i.atoms.net_wm_window_type_desktop,
        };
        conn.change_property32(xproto::PropMode::REPLACE, window, i.atoms.net_wm_window_type, xproto::AtomEnum::ATOM, &[wtype]).map_err(err)?;
        // WM_HINTS: sem o `input`, o gerenciador não dá o foco do teclado à camada.
        let input = u32::from(spec.keyboard);
        conn.change_property32(xproto::PropMode::REPLACE, window, i.atoms.wm_hints, i.atoms.wm_hints, &[1, input, 0, 0, 0, 0, 0, 0, 0]).map_err(err)?;

        let layer = Layer {
            session: self.clone(),
            window,
            rect: r,
            strut: spec.strut,
            colormap,
            gc: None,
            destroyed: false,
        };
        layer.write_strut()?;
        conn.map_window(window).map_err(err)?;
        if spec.kind == Kind::Desktop {
            // A área de trabalho fica por baixo de tudo, mesmo sem gerenciador de janelas.
            conn.configure_window(window, &xproto::ConfigureWindowAux::new().stack_mode(xproto::StackMode::BELOW)).map_err(err)?;
        }
        conn.flush().map_err(err)?;
        Ok(layer)
    }

    /// Lê os eventos pendentes (sem bloquear). Chame quando `fd` acordar.
    pub fn poll(&self) -> Result<Vec<Input>> {
        let i = &self.inner;
        let mut out = Vec::new();
        while let Some(event) = i.conn.poll_for_event().map_err(err)? {
            match event {
                Event::Expose(e) if e.count == 0 => out.push(Input::Expose { window: e.window }),
                Event::ButtonPress(e) => out.push(pointer(e.event, e.event_x, e.event_y, PointerKind::Press(e.detail))),
                Event::ButtonRelease(e) => out.push(pointer(e.event, e.event_x, e.event_y, PointerKind::Release(e.detail))),
                Event::MotionNotify(e) => out.push(pointer(e.event, e.event_x, e.event_y, PointerKind::Motion)),
                Event::EnterNotify(e) => out.push(pointer(e.event, e.event_x, e.event_y, PointerKind::Enter)),
                Event::LeaveNotify(e) => out.push(pointer(e.event, e.event_x, e.event_y, PointerKind::Leave)),
                Event::KeyPress(e) => {
                    let keysym = self.keysym_of(e.detail, 0)?;
                    out.push(Input::Key { window: e.event, keysym, pressed: true });
                }
                Event::KeyRelease(e) => {
                    let keysym = self.keysym_of(e.detail, 0)?;
                    out.push(Input::Key { window: e.event, keysym, pressed: false });
                }
                _ => {}
            }
        }
        i.conn.flush().map_err(err)?;
        Ok(out)
    }

    /// Símbolo (keysym) de uma tecla do servidor na coluna `col` (0 sem Shift).
    fn keysym_of(&self, keycode: u8, col: u8) -> Result<u32> {
        let i = &self.inner;
        let count = i.max_keycode - i.min_keycode + 1;
        let reply = i.conn.get_keyboard_mapping(i.min_keycode, count).map_err(err)?.reply().map_err(err)?;
        let per = reply.keysyms_per_keycode as usize;
        if per == 0 || keycode < i.min_keycode {
            return Ok(0);
        }
        let idx = (keycode - i.min_keycode) as usize * per + col as usize;
        Ok(reply.keysyms.get(idx).copied().unwrap_or(0))
    }

    /// Digita a tecla de `keysym` com XTEST, se o layout do servidor tem essa tecla
    /// (`Ok(false)` se não tem). `ctrl` segura o Control_L em volta.
    pub fn type_keysym(&self, keysym: u32, ctrl: bool) -> Result<bool> {
        let i = &self.inner;
        if !i.xtest {
            return Err(Error("o servidor não tem XTEST".into()));
        }
        let count = i.max_keycode - i.min_keycode + 1;
        let reply = i.conn.get_keyboard_mapping(i.min_keycode, count).map_err(err)?.reply().map_err(err)?;
        let per = reply.keysyms_per_keycode as usize;
        let mut found = None;
        for (n, chunk) in reply.keysyms.chunks(per.max(1)).enumerate() {
            if let Some(col) = chunk.iter().position(|&s| s == keysym) {
                found = Some((i.min_keycode + n as u8, col));
                break;
            }
        }
        let Some((keycode, col)) = found else { return Ok(false) };
        let control = if ctrl { self.find_keycode(0xffe3)? } else { None };
        let shift = if col == 1 { self.find_keycode(0xffe1)? } else { None };
        if let Some(k) = control {
            self.fake(xproto::KEY_PRESS_EVENT, k)?;
        }
        if let Some(k) = shift {
            self.fake(xproto::KEY_PRESS_EVENT, k)?;
        }
        self.fake(xproto::KEY_PRESS_EVENT, keycode)?;
        self.fake(xproto::KEY_RELEASE_EVENT, keycode)?;
        if let Some(k) = shift {
            self.fake(xproto::KEY_RELEASE_EVENT, k)?;
        }
        if let Some(k) = control {
            self.fake(xproto::KEY_RELEASE_EVENT, k)?;
        }
        i.conn.flush().map_err(err)?;
        Ok(true)
    }

    /// Digita um caractere que o layout do servidor não tem: usa uma tecla livre, com o
    /// símbolo posto nela por um instante (e o mapeamento volta depois). `Ok(false)` se
    /// não há tecla livre.
    pub fn type_keysym_remapped(&self, keysym: u32) -> Result<bool> {
        let i = &self.inner;
        if !i.xtest {
            return Err(Error("o servidor não tem XTEST".into()));
        }
        let count = i.max_keycode - i.min_keycode + 1;
        let reply = i.conn.get_keyboard_mapping(i.min_keycode, count).map_err(err)?.reply().map_err(err)?;
        let per = reply.keysyms_per_keycode as usize;
        let free = reply
            .keysyms
            .chunks(per.max(1))
            .enumerate()
            .rev()
            .find(|(_, chunk)| chunk.iter().all(|&s| s == 0))
            .map(|(n, _)| i.min_keycode + n as u8);
        let Some(keycode) = free else { return Ok(false) };
        let mut syms = vec![0u32; per.max(1)];
        syms[0] = keysym;
        i.conn.change_keyboard_mapping(1, keycode, per.max(1) as u8, &syms).map_err(err)?;
        i.conn.flush().map_err(err)?;
        // Os apps leem o mapeamento na chegada do MappingNotify; uma pausa curta basta.
        std::thread::sleep(std::time::Duration::from_millis(15));
        self.fake(xproto::KEY_PRESS_EVENT, keycode)?;
        self.fake(xproto::KEY_RELEASE_EVENT, keycode)?;
        i.conn.flush().map_err(err)?;
        std::thread::sleep(std::time::Duration::from_millis(15));
        let zero = vec![0u32; per.max(1)];
        i.conn.change_keyboard_mapping(1, keycode, per.max(1) as u8, &zero).map_err(err)?;
        i.conn.flush().map_err(err)?;
        Ok(true)
    }

    fn find_keycode(&self, keysym: u32) -> Result<Option<u8>> {
        let i = &self.inner;
        let count = i.max_keycode - i.min_keycode + 1;
        let reply = i.conn.get_keyboard_mapping(i.min_keycode, count).map_err(err)?.reply().map_err(err)?;
        let per = reply.keysyms_per_keycode.max(1) as usize;
        Ok(reply.keysyms.chunks(per).position(|c| c.contains(&keysym)).map(|n| i.min_keycode + n as u8))
    }

    fn fake(&self, kind: u8, keycode: u8) -> Result<()> {
        let i = &self.inner;
        i.conn.xtest_fake_input(kind, keycode, CURRENT_TIME, NONE, 0, 0, 0).map_err(err)?;
        Ok(())
    }
}

fn intern(conn: &RustConnection, name: &str) -> Result<xproto::Atom> {
    Ok(conn.intern_atom(false, name.as_bytes()).map_err(err)?.reply().map_err(err)?.atom)
}

fn pointer(window: Window, x: i16, y: i16, kind: PointerKind) -> Input {
    Input::Pointer { window, x: x as f64, y: y as f64, kind }
}

/// Uma camada X11: a janela, o quadro e o que ela reserva na tela.
pub struct Layer {
    session: Session,
    window: Window,
    rect: Rect,
    strut: Option<(Edge, u32)>,
    colormap: Option<u32>,
    gc: Option<u32>,
    destroyed: bool,
}

impl Layer {
    /// Id da janela (para casar com o `window` dos eventos).
    pub fn window(&self) -> u32 {
        self.window
    }

    /// Posição e tamanho atuais.
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// Move e redimensiona a janela.
    pub fn set_rect(&mut self, rect: Rect) -> Result<()> {
        let conn = &self.session.inner.conn;
        let aux = xproto::ConfigureWindowAux::new()
            .x(rect.x)
            .y(rect.y)
            .width(rect.w.max(1))
            .height(rect.h.max(1));
        conn.configure_window(self.window, &aux).map_err(err)?;
        conn.flush().map_err(err)?;
        self.rect = rect;
        self.write_strut()
    }

    /// Forma visível da janela (SHAPE de limite): só a parte `region` aparece. `None`
    /// volta a janela inteira.
    pub fn set_bounding_region(&mut self, region: Option<Rect>) -> Result<()> {
        self.shape(shape::SK::BOUNDING, region)
    }

    /// Área que recebe o ponteiro (SHAPE de entrada): fora dela, o clique passa para
    /// a janela de baixo. `None` volta a janela inteira.
    pub fn set_input_region(&mut self, region: Option<Rect>) -> Result<()> {
        self.shape(shape::SK::INPUT, region)
    }

    fn shape(&mut self, kind: shape::SK, region: Option<Rect>) -> Result<()> {
        let i = &self.session.inner;
        if !i.shape {
            return Err(Error("o servidor não tem SHAPE".into()));
        }
        match region {
            Some(r) => {
                let rects = [xproto::Rectangle { x: r.x as i16, y: r.y as i16, width: r.w as u16, height: r.h as u16 }];
                i.conn.shape_rectangles(shape::SO::SET, kind, xproto::ClipOrdering::UNSORTED, self.window, 0, 0, &rects).map_err(err)?;
            }
            None => {
                i.conn.shape_mask(shape::SO::SET, kind, self.window, 0, 0, NONE).map_err(err)?;
            }
        }
        i.conn.flush().map_err(err)?;
        Ok(())
    }

    /// Prende o ponteiro para esta camada (menus). `false` se o servidor recusou.
    pub fn grab_pointer(&self) -> bool {
        let i = &self.session.inner;
        let mask = xproto::EventMask::BUTTON_PRESS | xproto::EventMask::BUTTON_RELEASE | xproto::EventMask::POINTER_MOTION;
        let status = i
            .conn
            .grab_pointer(false, self.window, mask, xproto::GrabMode::ASYNC, xproto::GrabMode::ASYNC, NONE, NONE, CURRENT_TIME)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.status);
        let _ = i.conn.flush();
        status == Some(xproto::GrabStatus::SUCCESS)
    }

    /// Solta o ponteiro preso por `grab_pointer`.
    pub fn ungrab_pointer(&self) {
        let i = &self.session.inner;
        let _ = i.conn.ungrab_pointer(CURRENT_TIME);
        let _ = i.conn.flush();
    }

    /// Envia o quadro. O `pixmap` tem o tamanho da janela; sem alfa (ou sem o visual
    /// de 32 bits), as partes transparentes compõem sobre `background`.
    pub fn present(&mut self, pixmap: &Pixmap, background: [u8; 3]) -> Result<()> {
        let (w, h) = (pixmap.width(), pixmap.height());
        if w != self.rect.w.max(1) || h != self.rect.h.max(1) {
            return Err(Error(format!("quadro de {w}x{h} para uma camada de {}x{}", self.rect.w, self.rect.h)));
        }
        let i = &self.session.inner;
        let depth = if self.colormap.is_some() { 32 } else { i.root_depth };
        let gc = match self.gc {
            Some(gc) => gc,
            None => {
                let gc = i.conn.generate_id().map_err(err)?;
                i.conn.create_gc(gc, self.window, &xproto::CreateGCAux::new()).map_err(err)?;
                self.gc = Some(gc);
                gc
            }
        };
        let argb = self.colormap.is_some();
        let src = pixmap.data(); // RGBA premultiplicado
        let row = w as usize * 4;
        // Uma requisição PutImage cabe em ~256 KiB: as linhas vão em pedaços.
        let rows_per = (200_000 / row.max(1)).max(1);
        let mut y0 = 0usize;
        let mut buf = Vec::with_capacity(rows_per * row);
        while y0 < h as usize {
            let n = rows_per.min(h as usize - y0);
            buf.clear();
            for y in y0..y0 + n {
                for px in src[y * row..(y + 1) * row].chunks_exact(4) {
                    let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
                    if argb {
                        buf.extend_from_slice(&[b, g, r, a]);
                    } else {
                        let k = 255 - a as u16;
                        let mix = |c: u8, bg: u8| (c as u16 + (bg as u16 * k + 127) / 255).min(255) as u8;
                        buf.extend_from_slice(&[mix(b, background[2]), mix(g, background[1]), mix(r, background[0]), 0]);
                    }
                }
            }
            i.conn
                .put_image(xproto::ImageFormat::Z_PIXMAP, self.window, gc, w as u16, n as u16, 0, y0 as i16, 0, depth, &buf)
                .map_err(err)?;
            y0 += n;
        }
        i.conn.flush().map_err(err)?;
        Ok(())
    }

    /// Dica `_NET_WM_STRUT_PARTIAL` (e `_NET_WM_STRUT`) com a borda reservada.
    fn write_strut(&self) -> Result<()> {
        let i = &self.session.inner;
        let Some((edge, size)) = self.strut else {
            return Ok(());
        };
        let r = self.rect;
        let (x0, x1) = (r.x.max(0) as u32, (r.x + r.w as i32 - 1).max(0) as u32);
        let (y0, y1) = (r.y.max(0) as u32, (r.y + r.h as i32 - 1).max(0) as u32);
        // left, right, top, bottom, e o intervalo de cada uma (início e fim).
        let mut partial = [0u32; 12];
        let mut simple = [0u32; 4];
        match edge {
            Edge::Left => {
                partial[0] = size;
                partial[4] = y0;
                partial[5] = y1;
                simple[0] = size;
            }
            Edge::Right => {
                partial[1] = size;
                partial[6] = y0;
                partial[7] = y1;
                simple[1] = size;
            }
            Edge::Top => {
                partial[2] = size;
                partial[8] = x0;
                partial[9] = x1;
                simple[2] = size;
            }
            Edge::Bottom => {
                partial[3] = size;
                partial[10] = x0;
                partial[11] = x1;
                simple[3] = size;
            }
        }
        i.conn.change_property32(xproto::PropMode::REPLACE, self.window, i.atoms.net_wm_strut_partial, xproto::AtomEnum::CARDINAL, &partial).map_err(err)?;
        i.conn.change_property32(xproto::PropMode::REPLACE, self.window, i.atoms.net_wm_strut, xproto::AtomEnum::CARDINAL, &simple).map_err(err)?;
        i.conn.flush().map_err(err)?;
        Ok(())
    }

    /// Destrói a janela (e o contexto gráfico e o colormap dela).
    pub fn destroy(mut self) -> Result<()> {
        self.free()
    }

    fn free(&mut self) -> Result<()> {
        if self.destroyed {
            return Ok(());
        }
        self.destroyed = true;
        let i = &self.session.inner;
        if let Some(gc) = self.gc.take() {
            let _ = i.conn.free_gc(gc);
        }
        i.conn.destroy_window(self.window).map_err(err)?;
        if let Some(cmap) = self.colormap.take() {
            let _ = i.conn.free_colormap(cmap);
        }
        i.conn.flush().map_err(err)?;
        Ok(())
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        let _ = self.free();
    }
}
