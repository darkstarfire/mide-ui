//! Retângulo de seleção (apertar no vazio e arrastar), como o da mesa e do Finder no
//! Mac: um só componente para todos os apps, para o desenho e o comportamento serem
//! iguais em todo lugar (a área de trabalho e o conteúdo do Arquivos usam este).
//!
//! O app guarda um [`SelectionBand`] e chama [`SelectionBand::update`] a cada quadro,
//! depois de desenhar os itens; com o retângulo devolvido, escolhe os itens que ele
//! toca. O desenho sai na cor dos nomes dos ladrilhos (`Theme::tile_text`): sobre o
//! papel de parede, véu claro com borda branca; numa janela clara, cinza.

use super::ui::Ui;

/// Estado do retângulo de seleção (um por área que seleciona).
#[derive(Debug, Default, Clone)]
pub struct SelectionBand {
    /// Onde o botão foi apertado (coordenadas da área).
    press: Option<(f32, f32)>,
    /// Início e ponta, depois de o ponteiro andar mais que [`SelectionBand::SLOP`].
    rect: Option<((f32, f32), (f32, f32))>,
    /// O retângulo acabou de ser solto neste quadro (o clique no fundo que fecha um
    /// arrasto não tira a seleção).
    finished: bool,
}

impl SelectionBand {
    /// Quanto o ponteiro anda antes de virar retângulo (abaixo disso é clique).
    pub const SLOP: f32 = 4.0;

    /// Um quadro: começa no aperto dentro de `area` (só no vazio: `can_start` é falso
    /// sobre um item, com menu aberto ou durante outro arrasto; o arrasto da divisória
    /// de colunas do cabeçalho nunca começa um retângulo), segue o ponteiro, desenha e
    /// termina ao soltar. Devolve o retângulo (x, y, largura, altura) enquanto arrasta.
    pub fn update(&mut self, ui: &mut Ui, area: (f32, f32, f32, f32), can_start: bool) -> Option<(f32, f32, f32, f32)> {
        self.finished = false;
        let input = ui.input();
        let (pressed, down) = (input.pressed, input.down);
        let pointer = ui.pointer();
        if pressed && can_start && !ui.header_dragging() && ui.menu_open().is_none() {
            if let Some(p) = pointer.filter(|_| ui.over(area)) {
                self.press = Some(p);
                self.rect = None;
            }
        }
        if !down {
            self.finished = self.rect.is_some();
            self.press = None;
            self.rect = None;
            return None;
        }
        // Um arrasto de divisória que começou depois (no mesmo aperto) cancela.
        if ui.header_dragging() {
            self.press = None;
            self.rect = None;
            return None;
        }
        let (start, p) = (self.press?, pointer?);
        if self.rect.is_none() && ((p.0 - start.0).powi(2) + (p.1 - start.1).powi(2)).sqrt() > Self::SLOP {
            self.rect = Some((start, p));
        }
        let rect = self.rect.as_mut()?;
        rect.1 = p;
        let (a, b) = *rect;
        let r = (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs().max(1.0), (a.1 - b.1).abs().max(1.0));
        Self::paint(ui, r);
        ui.request_repaint();
        Some(r)
    }

    /// Está arrastando um retângulo agora.
    pub fn active(&self) -> bool {
        self.rect.is_some()
    }

    /// O botão está apertado depois de um aperto no vazio (o retângulo pode começar).
    pub fn pressed(&self) -> bool {
        self.press.is_some()
    }

    /// O retângulo foi solto neste quadro.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Esquece o aperto (o app começou outra coisa com ele, como arrastar itens).
    pub fn cancel(&mut self) {
        self.press = None;
        self.rect = None;
    }

    /// Desenho do retângulo: véu translúcido e borda fina, quadrados como no Mac.
    pub fn paint(ui: &mut Ui, (x, y, w, h): (f32, f32, f32, f32)) {
        let [r, g, b, _] = ui.theme().tile_text;
        let light = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000 > 128;
        // Claro sobre fundo escuro/foto (como na mesa do Mac), cinza sobre janela clara.
        let (fill, line) = if light { (40, 190) } else { (26, 110) };
        ui.paint(|canvas, _| {
            use crate::ui::draw::fill_rounded;
            use tiny_skia::Color;
            fill_rounded(canvas, x, y, w, h, 0.0, Color::from_rgba8(r, g, b, fill));
            let border = Color::from_rgba8(r, g, b, line);
            for (rx, ry, rw, rh) in [(x, y, w, 1.0), (x, y + h - 1.0, w, 1.0), (x, y + 1.0, 1.0, h - 2.0), (x + w - 1.0, y + 1.0, 1.0, h - 2.0)] {
                fill_rounded(canvas, rx, ry, rw.max(0.0), rh.max(0.0), 0.0, border);
            }
        });
    }
}

/// O retângulo `r` toca `item` (os dois em (x, y, largura, altura)).
pub fn band_touches(r: (f32, f32, f32, f32), item: (f32, f32, f32, f32)) -> bool {
    item.0 < r.0 + r.2 && item.0 + item.2 > r.0 && item.1 < r.1 + r.3 && item.1 + item.3 > r.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toque() {
        assert!(band_touches((0.0, 0.0, 10.0, 10.0), (5.0, 5.0, 10.0, 10.0)));
        assert!(!band_touches((0.0, 0.0, 10.0, 10.0), (10.0, 0.0, 5.0, 5.0)));
        assert!(!band_touches((0.0, 0.0, 10.0, 10.0), (0.0, 20.0, 5.0, 5.0)));
    }
}
