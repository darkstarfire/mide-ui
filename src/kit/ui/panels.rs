//! Painéis e janelas: divisórias arrastáveis (`split`), barra lateral de vidro com a
//! largura do app (`sidebar_panel`), vidro em qualquer retângulo (`glass_fill`),
//! botões de ícone com dica (`tool_button`, `tool_group`), o ponteiro pedido pelo
//! app (`set_cursor`) e as janelas filhas (`open_window`).

use super::{color, fill_rounded, Paint, Response, Stroke, Ui, LIGHT_SIZE};
use crate::kit::app::Window;

/// Forma do ponteiro pedida por um componente (`Ui::set_cursor`). Vale enquanto o
/// componente pedir, a cada quadro; sem pedido, a seta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Default,
    /// Mão: algo clicável que não parece botão (link).
    Pointer,
    /// Barra de texto.
    Text,
    /// Redimensionar na horizontal (divisória entre painéis lado a lado).
    ResizeHorizontal,
    /// Redimensionar na vertical (divisória entre painéis um sobre o outro).
    ResizeVertical,
    /// Mover (mãozinha fechada ou setas em cruz, conforme o tema).
    Move,
    /// Não permitido.
    NotAllowed,
}

/// Um botão de um grupo de ferramentas (`Ui::tool_group`): ícone, dica (obrigatória),
/// marcado e ativo.
#[derive(Debug, Clone, Copy)]
pub struct Tool<'a> {
    pub icon: &'a str,
    pub tip: &'a str,
    pub on: bool,
    pub enabled: bool,
}

impl<'a> Tool<'a> {
    /// Botão ativo e não marcado. A dica aparece com o ponteiro parado sobre ele.
    pub fn new(icon: &'a str, tip: &'a str) -> Self {
        Self { icon, tip, on: false, enabled: true }
    }

    /// Marcado (painel aberto, modo escolhido).
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    /// Desativado (cinza, sem clique).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Largura da faixa da divisória que pega o ponteiro (metade de cada lado da linha).
const DIVIDER_HIT: f32 = 8.0;

impl<'a> Ui<'a> {
    /// Pede a forma do ponteiro enquanto ele estiver sobre esta janela (vale só neste
    /// quadro: o componente pede de novo a cada quadro em que o ponteiro está nele).
    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.ctx.cursor = Some(cursor);
    }

    // ---- Janelas filhas -------------------------------------------------------------

    /// Abre uma janela filha (Preferências, um inspetor): uma janela própria que
    /// pertence à principal (`xdg_toplevel.set_parent`: o compositor a mantém por cima
    /// da principal e a centra sobre ela, e ela fecha junto com o app). O conteúdo é
    /// desenhado por [`App::window_view`](crate::kit::App::window_view) com o mesmo
    /// `id`. Pedir de novo uma janela já aberta não faz nada. O `app_id` é sempre o da
    /// principal (a dock mostra um ícone só); título, tamanhos, `full_size_content`,
    /// `fixed_size`, `glass` e `accepts_files` de `window` valem para a filha.
    ///
    /// ```ignore
    /// if ui.tool_button("preferences-system-symbolic", "Settings").clicked() {
    ///     ui.open_window("prefs", Window::new("Preferências").size(520, 440).glass());
    /// }
    /// ```
    pub fn open_window(&mut self, id: &str, window: Window) {
        if self.is_window_open(id) {
            return;
        }
        self.ctx.close_windows.retain(|w| w != id);
        self.ctx.open_windows.push((id.to_string(), window));
    }

    /// Fecha a janela filha `id` (dentro do `window_view` dela, `ui.close()` faz o
    /// mesmo). O app recebe [`App::window_closed`](crate::kit::App::window_closed).
    pub fn close_window(&mut self, id: &str) {
        self.ctx.open_windows.retain(|(w, _)| w != id);
        if !self.ctx.close_windows.iter().any(|w| w == id) {
            self.ctx.close_windows.push(id.to_string());
        }
    }

    /// A janela filha `id` está aberta (ou foi pedida neste quadro)?
    pub fn is_window_open(&self, id: &str) -> bool {
        let closing = self.ctx.close_windows.iter().any(|w| w == id);
        !closing && (self.ctx.windows_open.iter().any(|w| w == id) || self.ctx.open_windows.iter().any(|(w, _)| w == id))
    }

    /// Qual janela este `view` desenha: `None` na principal, `Some(id)` numa filha.
    pub fn window_id(&self) -> Option<&str> {
        self.ctx.window_id.as_deref()
    }

    // ---- Vidro ----------------------------------------------------------------------

    /// Fundo de vidro em `rect` (nesta área), com cantos `radius`: o compositor
    /// desfoca o que está atrás da janela e o app pinta o véu do vidro de painel
    /// (`docs/design.md`, "Vidro"). Com o vidro desligado nas Configurações (ou fora do
    /// mide), pinta a cor opaca da barra lateral e devolve `false`. A janela acompanha
    /// a opção nas Configurações sozinha.
    pub fn glass_fill(&mut self, rect: (f32, f32, f32, f32), radius: f32) -> bool {
        let t = self.ctx.theme.clone();
        self.ctx.glass_capable = true;
        if self.ctx.glass_enabled() {
            crate::ui::draw::clear_rounded(self.canvas, rect.0, rect.1, rect.2, rect.3, radius);
            crate::ui::draw::paint_rounded_solid(self.canvas, rect.0, rect.1, rect.2, rect.3, radius, t.sidebar_glass);
            self.ctx.glass.push((self.offset.0 + rect.0, self.offset.1 + rect.1, rect.2, rect.3, radius));
            true
        } else {
            fill_rounded(self.canvas, rect.0, rect.1, rect.2, rect.3, radius, color(t.sidebar));
            false
        }
    }

    // ---- Divisórias -----------------------------------------------------------------

    /// Dois painéis lado a lado, com a divisória arrastável entre eles: `left` recebe
    /// `*left_width` de largura e `right` o resto, os dois até o fim da área (cada um
    /// recortado no seu retângulo, com as coordenadas começando no canto dele). O
    /// ponteiro vira o de redimensionar sobre a divisória. A largura fica entre `min` e
    /// `max` (e deixa pelo menos 80 para a direita). A resposta tem `changed` enquanto
    /// arrasta e `submitted` quando o arrasto termina (a hora de guardar a largura).
    ///
    /// ```ignore
    /// let r = ui.split(&mut self.largura, 160.0, 480.0, |ui| self.arvore(ui), |ui| self.editor(ui));
    /// if r.submitted() { self.config.largura = self.largura; self.config.gravar(); }
    /// ```
    pub fn split(&mut self, left_width: &mut f32, min: f32, max: f32, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) -> Response {
        let (x, y, w) = (self.x, self.cursor_y, self.width);
        let h = (self.visible.1 - y).max(1.0);
        let hi = max.min(w - 80.0).max(min);
        let lw = left_width.clamp(min, hi);
        *left_width = lw;
        let r = self.divider((x + lw - DIVIDER_HIT / 2.0, y, DIVIDER_HIT, h), true, left_width, min, hi, 1.0);
        let lw = *left_width;
        self.clip((x, y, lw, h), left);
        self.clip((x + lw + 1.0, y, (w - lw - 1.0).max(1.0), h), right);
        let t = self.ctx.theme.clone();
        fill_rounded(self.canvas, x + lw, y, 1.0, h, 0.0, color(t.line));
        self.cursor_y = y + h;
        r
    }

    /// Dois painéis um sobre o outro, com a divisória arrastável: `bottom` recebe
    /// `*bottom_height` de altura (o terminal embaixo do editor, por exemplo) e `top`
    /// o resto. Como [`Ui::split`], com a altura entre `min` e `max` (e pelo menos 60
    /// para o de cima).
    pub fn split_vertical(&mut self, bottom_height: &mut f32, min: f32, max: f32, top: impl FnOnce(&mut Ui), bottom: impl FnOnce(&mut Ui)) -> Response {
        let (x, y, w) = (self.x, self.cursor_y, self.width);
        let h = (self.visible.1 - y).max(1.0);
        let hi = max.min(h - 60.0).max(min);
        *bottom_height = bottom_height.clamp(min, hi);
        let edge = y + h - *bottom_height;
        let r = self.divider((x, edge - DIVIDER_HIT / 2.0, w, DIVIDER_HIT), false, bottom_height, min, hi, -1.0);
        let bh = *bottom_height;
        let edge = y + h - bh;
        self.clip((x, y, w, (edge - y).max(1.0)), top);
        self.clip((x, edge + 1.0, w, (bh - 1.0).max(1.0)), bottom);
        let t = self.ctx.theme.clone();
        fill_rounded(self.canvas, x, edge, w, 1.0, 0.0, color(t.line));
        self.cursor_y = y + h;
        r
    }

    /// Barra lateral de vidro com a largura escolhida pelo app e a divisória
    /// arrastável (a árvore do projeto de um editor, as pastas de um app de notas):
    /// `side` desenha dentro da barra e `content` à direita, até o fim da área. Numa
    /// janela de conteúdo inteiro (Tahoe), a barra flutua solta da borda, com os botões
    /// da janela no topo (o conteúdo dela começa abaixo deles) e o vazio arrastando a
    /// janela, como a do Arquivos; numa janela comum, encosta na borda com uma linha à
    /// direita. O vidro segue Configurações → Efeitos (desligado, fica opaca). A
    /// resposta é a da divisória (ver [`Ui::split`]).
    #[allow(clippy::too_many_arguments)]
    pub fn sidebar_panel(&mut self, width: &mut f32, min: f32, max: f32, side: impl FnOnce(&mut Ui), content: impl FnOnce(&mut Ui)) -> Response {
        let t = self.ctx.theme.clone();
        let (x, y, w) = (self.x, self.cursor_y, self.width);
        let h = (self.visible.1 - y).max(1.0);
        let hi = max.min(w - 80.0).max(min);
        *width = width.clamp(min, hi);
        let r = self.divider((x + *width - DIVIDER_HIT / 2.0, y, DIVIDER_HIT, h), true, width, min, hi, 1.0);
        let sw = *width;
        let tahoe = self.ctx.content_window;
        let inset = if tahoe { crate::kit::theme::INSET_FLOAT } else { 0.0 };
        let radius = if tahoe { crate::kit::theme::RADIUS_SIDEBAR } else { 0.0 };
        let panel = (x + inset, y + inset, sw - inset, h - 2.0 * inset);
        if tahoe {
            fill_rounded(self.canvas, x, y, sw, h, 0.0, color(t.background));
        }
        self.glass_fill(panel, radius);
        let top = if tahoe {
            if let Some(path) = crate::ui::draw::rounded_rect(panel.0 + 0.5, panel.1 + 0.5, panel.2 - 1.0, panel.3 - 1.0, radius - 0.5) {
                let mut paint = Paint { anti_alias: true, ..Default::default() };
                paint.set_color(color(if t.dark { [255, 255, 255, 34] } else { [0, 0, 0, 20] }));
                self.canvas.stroke_path(&path, &paint, &Stroke { width: 1.0, ..Default::default() }, crate::ui::draw::tf(), None);
            }
            self.window_controls(x + 20.0, y + (crate::kit::theme::TOOLBAR_HEIGHT - LIGHT_SIZE) / 2.0);
            y + crate::kit::theme::TOOLBAR_HEIGHT
        } else {
            fill_rounded(self.canvas, x + sw - 1.0, y, 1.0, h, 0.0, color(t.line));
            y
        };
        // Conteúdo da barra (recortado nela, sem passar por cima dos cantos).
        let bottom = panel.1 + panel.3 - radius * 0.3;
        self.clip((panel.0, top, panel.2 - (!tahoe) as u8 as f32, (bottom - top).max(1.0)), side);
        if tahoe {
            self.window_drag_area(panel);
        }
        self.clip((x + sw, y, (w - sw).max(1.0), h), content);
        self.cursor_y = y + h;
        r
    }

    /// A divisória: a faixa `hit` pega o ponteiro, arrastar muda `value` (com `sign`
    /// +1 cresce para a direita/baixo, -1 para a esquerda/cima) entre `min` e `max`.
    fn divider(&mut self, hit: (f32, f32, f32, f32), horizontal: bool, value: &mut f32, min: f32, max: f32, sign: f32) -> Response {
        let id = self.ctx.id();
        let mut r = self.interact(id, hit);
        let along = |p: (f32, f32)| if horizontal { p.0 } else { p.1 };
        let pointer = self.pointer();
        if r.hovered
            && self.ctx.input.pressed
            && let Some(p) = pointer
        {
            self.ctx.memory.split_drag = Some((id, along(p), *value));
        }
        let mut dragging = false;
        if let Some((drag, start, start_value)) = self.ctx.memory.split_drag
            && drag == id
        {
            if self.ctx.input.down {
                dragging = true;
                if let Some(p) = pointer {
                    let v = (start_value + sign * (along(p) - start)).clamp(min, max.max(min));
                    if (v - *value).abs() > 0.01 {
                        *value = v;
                        r.changed = true;
                        self.ctx.repaint = true;
                    }
                }
            } else {
                self.ctx.memory.split_drag = None;
                r.submitted = true;
            }
        }
        // Clique simples na divisória não vale como clique em nada.
        r.clicked = false;
        r.double_clicked = false;
        if r.hovered || dragging {
            self.set_cursor(if horizontal { Cursor::ResizeHorizontal } else { Cursor::ResizeVertical });
        }
        r
    }

    // ---- Botões de ícone com dica ---------------------------------------------------

    /// Botão só de ícone, com a dica obrigatória (aparece com o ponteiro parado sobre
    /// ele, I2). A dica passa por [`tr`](crate::kit::tr), o ponto de entrada da
    /// tradução (I1). Se o ícone faltar, aparece a primeira letra da dica.
    pub fn tool_button(&mut self, icon: &str, tip: &str) -> Response {
        self.tool_button_with(Tool::new(icon, tip))
    }

    /// Como [`Ui::tool_button`], marcado quando `on` (painel aberto, modo escolhido).
    pub fn tool_toggle(&mut self, icon: &str, tip: &str, on: bool) -> Response {
        self.tool_button_with(Tool::new(icon, tip).on(on))
    }

    /// Botão de ícone com dica, com todas as opções de [`Tool`].
    pub fn tool_button_with(&mut self, tool: Tool) -> Response {
        let tip = crate::kit::tr(tool.tip);
        let fallback: String = tip.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
        self.toggled = tool.on;
        let r = self.icon_button(tool.icon, &fallback, tool.enabled);
        self.toggled = false;
        self.tooltip(&r, &tip);
        r
    }

    /// Grupo de botões de ícone numa cápsula, para a barra de ferramentas do Tahoe
    /// (como [`Ui::toolbar_buttons`]), cada um com a dica obrigatória. Devolve o índice
    /// clicado.
    ///
    /// ```ignore
    /// match ui.tool_group(&[Tool::new("document-open-symbolic", "Open File"), Tool::new("folder-open-symbolic", "Open Folder")]) {
    ///     Some(0) => self.abrir_arquivo(),
    ///     Some(1) => self.abrir_pasta(),
    ///     _ => {}
    /// }
    /// ```
    pub fn tool_group(&mut self, tools: &[Tool]) -> Option<usize> {
        let tips: Vec<String> = tools.iter().map(|t| crate::kit::tr(t.tip).into_owned()).collect();
        let fallbacks: Vec<String> = tips.iter().map(|t| t.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default()).collect();
        let items: Vec<(&str, &str, bool, bool)> = tools.iter().zip(&fallbacks).map(|(t, f)| (t.icon, f.as_str(), t.on, t.enabled)).collect();
        let (clicked, responses) = self.toolbar_buttons_impl(&items);
        for (r, tip) in responses.iter().zip(&tips) {
            self.tooltip(r, tip);
        }
        clicked
    }
}
