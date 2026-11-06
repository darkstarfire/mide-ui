//! Texto de várias linhas para editar (o documento do `Ui::text_area`): o texto, o
//! cursor e a seleção, desfazer e refazer, as contas de linha e coluna e as
//! decorações (cores, sublinhados e texto no fim da linha, do `Ui::text_area_with`).
//!
//! Lógica pura, testável sem janela. O desenho e as teclas ficam no `Ui::text_area`.
//! O histórico guarda cópias do texto (simples e suficiente para um editor mínimo);
//! um editor de arquivos grandes trocaria isso por uma estrutura própria.

use std::ops::Range;
use std::time::{Duration, Instant};

use super::{textedit::Edit, theme::Rgba};

/// Tipo de mudança, para juntar no histórico o que foi digitado em sequência (desfazer
/// volta uma palavra digitada de uma vez, e não letra por letra).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Typing,
    Deleting,
    Other,
}

/// Documento em edição.
#[derive(Debug, Clone, Default)]
pub struct TextDoc {
    pub text: String,
    pub edit: Edit,
    /// Rolagem vertical e horizontal, em pixels.
    pub scroll: f32,
    pub scroll_x: f32,
    /// Coluna desejada (em pixels) ao subir e descer linhas por textos curtos.
    pub goal_x: Option<f32>,
    undo: Vec<(String, Edit)>,
    redo: Vec<(String, Edit)>,
    last_change: Option<(Change, Instant)>,
    /// O texto mudou desde que foi aberto ou salvo.
    pub modified: bool,
    /// Aumenta a cada mudança do texto (para saber se mudou num quadro).
    pub version: u64,
    /// Cursores além do principal (vários cursores, como no VS Code e no Zed), cada um
    /// com a própria seleção. Digitar, apagar, as setas, colar e copiar valem para
    /// todos; Esc (no app) volta a um só.
    pub extra: Vec<Edit>,
}

impl TextDoc {
    pub fn new(text: String) -> Self {
        Self { text, ..Default::default() }
    }

    /// Guarda o estado antes de uma mudança. Mudanças do mesmo tipo em sequência
    /// (digitar várias letras) ficam num passo só do desfazer.
    pub fn before_change(&mut self, change: Change) {
        let now = Instant::now();
        let joins = change != Change::Other
            && self.last_change.is_some_and(|(kind, at)| kind == change && now.duration_since(at) < Duration::from_secs(1));
        if !joins {
            self.undo.push((self.text.clone(), self.edit));
            if self.undo.len() > 500 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_change = Some((change, now));
        self.modified = true;
        self.version += 1;
    }

    /// Desfaz a última mudança; devolve se havia o que desfazer.
    pub fn undo(&mut self) -> bool {
        let Some((text, edit)) = self.undo.pop() else {
            return false;
        };
        self.redo.push((std::mem::replace(&mut self.text, text), self.edit));
        self.edit = edit;
        self.extra.clear();
        self.last_change = None;
        self.modified = true;
        self.version += 1;
        true
    }

    /// Há o que desfazer (o menu Editar desativa "Desfazer" sem isso).
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Há o que refazer.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Refaz o que foi desfeito; devolve se havia o que refazer.
    pub fn redo(&mut self) -> bool {
        let Some((text, edit)) = self.redo.pop() else {
            return false;
        };
        self.undo.push((std::mem::replace(&mut self.text, text), self.edit));
        self.edit = edit;
        self.extra.clear();
        self.last_change = None;
        self.modified = true;
        self.version += 1;
        true
    }

    /// Começo de cada linha (índices de byte).
    pub fn line_starts(&self) -> Vec<usize> {
        line_starts(&self.text)
    }

    /// Escreve no cursor (no lugar da seleção), registrando no histórico. Com vários
    /// cursores, em todos.
    pub fn type_text(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.before_change(if s.chars().any(char::is_whitespace) { Change::Other } else { Change::Typing });
        self.each_cursor(|e, t| e.insert(t, s));
        self.goal_x = None;
    }

    /// Nova linha com o mesmo recuo da linha de cada cursor.
    pub fn newline(&mut self) {
        self.before_change(Change::Other);
        self.each_cursor(|e, t| {
            let at = e.selection().map_or(e.cursor, |(a, _)| a);
            let start = t[..at].rfind('\n').map_or(0, |i| i + 1);
            let indent: String = t[start..].chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            e.insert(t, &format!("\n{indent}"));
        });
        self.goal_x = None;
    }

    /// Há mais de um cursor.
    pub fn has_extra(&self) -> bool {
        !self.extra.is_empty()
    }

    /// Volta a um cursor só (o principal). Devolve se havia outros.
    pub fn collapse_cursors(&mut self) -> bool {
        let had = !self.extra.is_empty();
        self.extra.clear();
        had
    }

    /// Todos os cursores, o principal primeiro.
    pub fn cursors(&self) -> Vec<Edit> {
        std::iter::once(self.edit).chain(self.extra.iter().copied()).collect()
    }

    /// Aplica `f` em cada cursor, do último para o primeiro no texto: quando um muda o
    /// tamanho do texto, os que vêm depois dele (já feitos) andam junto. No fim, cursores
    /// que caíram no mesmo lugar viram um só. Não registra no histórico (quem chama
    /// registra antes, para tudo virar um passo só do desfazer).
    pub fn each_cursor(&mut self, mut f: impl FnMut(&mut Edit, &mut String)) {
        let mut all = self.cursors();
        let start = |e: &Edit| e.selection().map_or(e.cursor, |(a, _)| a);
        let mut order: Vec<usize> = (0..all.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(start(&all[i])));
        let mut done: Vec<usize> = Vec::new();
        for i in order {
            let len = self.text.len();
            let at = start(&all[i]);
            f(&mut all[i], &mut self.text);
            let delta = self.text.len() as isize - len as isize;
            if delta != 0 {
                let moved = |p: usize| (p as isize + delta).max(0) as usize;
                for &j in &done {
                    let e = &mut all[j];
                    if start(e) >= at {
                        e.cursor = moved(e.cursor);
                        e.anchor = e.anchor.map(moved);
                    }
                }
            }
            done.push(i);
        }
        for e in &mut all {
            e.clamp(&self.text);
        }
        self.edit = all[0];
        let mut extra: Vec<Edit> = Vec::new();
        for e in all.into_iter().skip(1) {
            if e.cursor != self.edit.cursor && !extra.iter().any(|x| x.cursor == e.cursor) {
                extra.push(e);
            }
        }
        self.extra = extra;
    }

    /// Cola em todos os cursores. Se o texto tem tantas linhas quanto os cursores (o
    /// que foi copiado de vários cursores), cada cursor recebe a sua, como no VS Code.
    pub fn paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n");
        self.before_change(Change::Other);
        let lines: Vec<&str> = text.split('\n').collect();
        let n = 1 + self.extra.len();
        if n > 1 && lines.len() == n {
            // Na ordem do texto: o primeiro cursor recebe a primeira linha.
            let mut by_pos: Vec<usize> = self.cursors().iter().map(|e| e.selection().map_or(e.cursor, |(a, _)| a)).collect();
            by_pos.sort();
            let pieces: Vec<(usize, String)> = by_pos.into_iter().zip(lines.iter().map(|l| l.to_string())).collect();
            self.each_cursor(|e, t| {
                let at = e.selection().map_or(e.cursor, |(a, _)| a);
                let piece = pieces.iter().find(|(p, _)| *p == at).map(|(_, s)| s.clone()).unwrap_or_default();
                e.insert(t, &piece);
            });
        } else {
            self.each_cursor(|e, t| e.insert(t, &text));
        }
        self.goal_x = None;
    }

    /// O texto escolhido em todos os cursores (um por linha, na ordem do texto), para
    /// copiar.
    pub fn selected_texts(&self) -> Option<String> {
        let mut parts: Vec<(usize, &str)> = self.cursors().iter().filter_map(|e| e.selection().map(|(a, b)| (a, &self.text[a..b]))).collect();
        parts.sort_by_key(|(a, _)| *a);
        (!parts.is_empty()).then(|| parts.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n"))
    }

    /// Põe (ou tira) um cursor em `at` (Alt+clique).
    pub fn toggle_cursor(&mut self, at: usize) {
        if let Some(i) = self.extra.iter().position(|e| e.cursor == at) {
            self.extra.remove(i);
        } else if self.edit.cursor != at {
            self.extra.push(Edit { cursor: at, anchor: None });
        }
    }

    /// Escolhe a próxima ocorrência do texto escolhido com mais um cursor (Ctrl+D):
    /// sem nada escolhido, escolhe a palavra do cursor. Devolve se escolheu algo.
    pub fn add_next_occurrence(&mut self) -> bool {
        let Some((a, b)) = self.edit.selection() else {
            let at = self.edit.cursor;
            self.edit.select_word(&self.text, at);
            return self.edit.selection().is_some();
        };
        let needle = self.text[a..b].to_string();
        // Depois do último cursor; no fim do texto, volta ao começo.
        let after = self.cursors().iter().map(|e| e.selection().map_or(e.cursor, |(_, z)| z)).max().unwrap_or(b);
        let taken: Vec<usize> = self.cursors().iter().filter_map(|e| e.selection().map(|(s, _)| s)).collect();
        let found = self.text[after..].find(&needle).map(|i| after + i).filter(|i| !taken.contains(i)).or_else(|| {
            self.text.match_indices(&needle).map(|(i, _)| i).find(|i| !taken.contains(i))
        });
        match found {
            Some(i) => {
                self.extra.push(Edit { cursor: i + needle.len(), anchor: Some(i) });
                true
            }
            None => false,
        }
    }

    /// Mais um cursor na linha de baixo (ou de cima) do último, na mesma coluna em
    /// caracteres (Ctrl+Alt+↓ e ↑).
    pub fn add_cursor_vertical(&mut self, down: bool) {
        let starts = self.line_starts();
        let edge = if down { self.cursors().into_iter().max_by_key(|e| e.cursor) } else { self.cursors().into_iter().min_by_key(|e| e.cursor) };
        let Some(edge) = edge else {
            return;
        };
        let line = line_of(&starts, edge.cursor);
        let target = if down { line + 1 } else { line.wrapping_sub(1) };
        if target >= starts.len() {
            return;
        }
        // A coluna do cursor principal (uma linha curta no meio não encurta as próximas).
        let main_line = line_of(&starts, self.edit.cursor);
        let col = self.text[starts[main_line]..self.edit.cursor].chars().count();
        let end = line_end(&self.text, &starts, target);
        let at = self.text[starts[target]..end].char_indices().nth(col).map_or(end, |(i, _)| starts[target] + i);
        self.toggle_cursor(at);
    }

    /// Move cada cursor uma linha para cima ou para baixo, na mesma coluna em
    /// caracteres (as setas com vários cursores).
    pub fn move_cursors_vertical(&mut self, down: bool, select: bool) {
        self.each_cursor(|e, t| {
            let starts = line_starts(t);
            let line = line_of(&starts, e.cursor);
            let to = if down { (line + 1).min(starts.len() - 1) } else { line.saturating_sub(1) };
            let col = t[starts[line]..e.cursor].chars().count();
            let end = line_end(t, &starts, to);
            let at = t[starts[to]..end].char_indices().nth(col).map_or(end, |(i, _)| starts[to] + i);
            e.move_to(at, select);
        });
    }
}

/// Começo de cada linha (índices de byte); sempre há pelo menos uma linha.
pub fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect()
}

/// Linha (a partir de 0) que contém a posição `at`.
pub fn line_of(starts: &[usize], at: usize) -> usize {
    starts.partition_point(|s| *s <= at).saturating_sub(1)
}

/// Fim da linha `line` (antes do `\n`).
pub fn line_end(text: &str, starts: &[usize], line: usize) -> usize {
    starts.get(line + 1).map_or(text.len(), |next| next - 1)
}

/// Posição na linha `line` mais perto de `x` pixels, medindo com `width` (largura do
/// começo da linha até um ponto). A largura do começo só cresce, então a busca é
/// binária: poucas medidas mesmo numa linha de milhares de caracteres (um JSON
/// minificado), em vez de uma por caractere.
pub fn index_in_line(text: &str, starts: &[usize], line: usize, x: f32, mut width: impl FnMut(&str) -> f32) -> usize {
    let (start, end) = (starts[line], line_end(text, starts, line));
    let content = &text[start..end];
    // Fim de cada caractere; o caractere k fica entre ends[k-1] (ou 0) e ends[k].
    let ends: Vec<usize> = content.char_indices().map(|(i, c)| i + c.len_utf8()).collect();
    let mut w = |k: usize| if k == 0 { 0.0 } else { width(&content[..ends[k - 1]]) };
    // O primeiro caractere cujo meio passa de x: o cursor fica antes dele.
    let (mut lo, mut hi) = (0, ends.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if x < (w(mid) + w(mid + 1)) / 2.0 {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    if lo == ends.len() { end } else { start + if lo == 0 { 0 } else { ends[lo - 1] } }
}

/// Decorações de uma área de texto (`Ui::text_area_with`), em índices de byte do
/// texto: trechos coloridos, trechos sublinhados (pontilhado na cor de perigo: os
/// erros) e um texto desenhado depois do fim de uma linha (o resultado das notas da
/// Calculadora). Ficam fora do texto: cursor, seleção, setas e clique não mudam.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decorations {
    pub colors: Vec<(Range<usize>, Rgba)>,
    pub underlines: Vec<Range<usize>>,
    /// (linha a partir de 0, texto, cor): o texto vem 4 pt depois do fim da linha;
    /// vários na mesma linha vêm na ordem da lista, com 4 pt entre eles. O que não cabe
    /// até a borda termina com "…".
    pub trailing: Vec<(usize, String, Rgba)>,
    /// O texto do fim da linha encostado na borda direita da área (os avisos de um
    /// editor de código), cortado com "…" se não couber; a área não rola para mostrá-lo.
    pub trailing_right: bool,
    /// Marcas na margem dos números (linha a partir de 0, cor): uma faixa fina na
    /// borda direita da margem, como as do Git nos editores (linha nova, mudada); com
    /// `true`, um triângulo pequeno entre as linhas (linhas apagadas acima desta).
    pub gutter: Vec<(usize, Rgba, bool)>,
}

/// O trecho `r` cortado na linha `start..end`, se ele vale: dentro do texto, nos
/// limites de caractere e não vazio depois do corte.
fn clip_to_line(text: &str, start: usize, end: usize, r: &Range<usize>) -> Option<(usize, usize)> {
    let ok = |i: usize| i <= text.len() && text.is_char_boundary(i);
    if r.start >= r.end || !ok(r.start) || !ok(r.end) {
        return None;
    }
    let (a, b) = (r.start.max(start), r.end.min(end));
    (a < b).then_some((a, b))
}

/// Pedaços da linha `start..end` por cor, em ordem e cobrindo a linha inteira:
/// (início, fim, cor), com `None` na cor normal do texto. Trechos fora do texto ou fora
/// do limite de um caractere são ignorados; quando dois se cobrem, vale o primeiro da
/// lista.
pub fn color_runs(text: &str, start: usize, end: usize, colors: &[(Range<usize>, Rgba)]) -> Vec<(usize, usize, Option<Rgba>)> {
    let spans: Vec<(usize, usize, Rgba)> = colors.iter().filter_map(|(r, c)| clip_to_line(text, start, end, r).map(|(a, b)| (a, b, *c))).collect();
    let mut cuts = vec![start, end];
    for (a, b, _) in &spans {
        cuts.push(*a);
        cuts.push(*b);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut runs: Vec<(usize, usize, Option<Rgba>)> = Vec::new();
    for pair in cuts.windows(2) {
        let (p, q) = (pair[0], pair[1]);
        let color = spans.iter().find(|(a, b, _)| *a <= p && q <= *b).map(|s| s.2);
        match runs.last_mut() {
            Some(last) if last.2 == color => last.1 = q,
            _ => runs.push((p, q, color)),
        }
    }
    runs
}

/// Os trechos do pedaço `a..b` (da linha que começa em `start`) que podem ser
/// desenhados sozinhos. O texto desenhado conta as paradas de tabulação do começo dele:
/// um pedaço do meio da linha com tabulação é cortado nela, e cada trecho vai para o x
/// medido do começo da linha (a tabulação em si não desenha nada). Um pedaço que começa
/// a linha fica inteiro (o Editor de Texto, sem cores, desenha a linha de uma vez).
pub fn tab_pieces(text: &str, start: usize, a: usize, b: usize) -> Vec<(usize, usize)> {
    if a == start || !text[a..b].contains('\t') {
        return vec![(a, b)];
    }
    let mut pieces = Vec::new();
    let mut from = a;
    for (i, _) in text[a..b].match_indices('\t') {
        if from < a + i {
            pieces.push((from, a + i));
        }
        from = a + i + 1;
    }
    if from < b {
        pieces.push((from, b));
    }
    pieces
}

/// Os sublinhados que caem na linha `start..end`, cortados nela (os inválidos saem).
/// As cores que tocam o trecho `from..to`: em ordem e sem sobreposição (como as do
/// realce), achadas por busca binária; senão, todas.
pub fn spans_in<T>(spans: &[(Range<usize>, T)], from: usize, to: usize) -> &[(Range<usize>, T)] {
    if !spans.windows(2).all(|w| w[0].0.end <= w[1].0.start) {
        return spans;
    }
    let a = spans.partition_point(|(r, _)| r.end < from);
    let b = spans.partition_point(|(r, _)| r.start <= to);
    &spans[a..b.max(a)]
}

/// Como [`spans_in`], para trechos sem cor (sublinhados).
pub fn ranges_in(ranges: &[Range<usize>], from: usize, to: usize) -> &[Range<usize>] {
    if !ranges.windows(2).all(|w| w[0].end <= w[1].start) {
        return ranges;
    }
    let a = ranges.partition_point(|r| r.end < from);
    let b = ranges.partition_point(|r| r.start <= to);
    &ranges[a..b.max(a)]
}

pub fn underline_runs(text: &str, start: usize, end: usize, underlines: &[Range<usize>]) -> Vec<(usize, usize)> {
    underlines.iter().filter_map(|r| clip_to_line(text, start, end, r)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varios_cursores_digitam_e_apagam_juntos() {
        let mut doc = TextDoc::new("ab\nab\nab".into());
        doc.edit = Edit { cursor: 2, anchor: None };
        doc.extra = vec![Edit { cursor: 5, anchor: None }, Edit { cursor: 8, anchor: None }];
        doc.type_text("X");
        assert_eq!(doc.text, "abX\nabX\nabX");
        assert_eq!(doc.cursors().iter().map(|e| e.cursor).collect::<Vec<_>>(), vec![3, 7, 11]);
        doc.before_change(Change::Deleting);
        doc.each_cursor(|e, t| {
            e.backspace(t, false);
        });
        assert_eq!(doc.text, "ab\nab\nab");
        // Desfazer volta o lote num passo só e deixa um cursor.
        doc.undo();
        assert_eq!(doc.text, "abX\nabX\nabX");
        assert!(!doc.has_extra());
    }

    #[test]
    fn proxima_ocorrencia_e_copiar_colar() {
        let mut doc = TextDoc::new("let a = a + a;".into());
        doc.edit = Edit { cursor: 4, anchor: None };
        assert!(doc.add_next_occurrence());
        assert_eq!(doc.edit.selection(), Some((4, 5)));
        assert!(doc.add_next_occurrence());
        assert!(doc.add_next_occurrence());
        assert_eq!(doc.extra.len(), 2);
        assert_eq!(doc.selected_texts().as_deref(), Some("a\na\na"));
        doc.type_text("bb");
        assert_eq!(doc.text, "let bb = bb + bb;");
        // Colar três linhas em três cursores: uma em cada.
        doc.paste("1\n2\n3");
        assert_eq!(doc.text, "let bb1 = bb2 + bb3;");
    }

    #[test]
    fn cursor_na_linha_de_baixo() {
        let mut doc = TextDoc::new("abcd\nxy\nefgh".into());
        doc.edit = Edit { cursor: 3, anchor: None };
        doc.add_cursor_vertical(true);
        // A linha de baixo é mais curta: o cursor vai ao fim dela.
        assert_eq!(doc.extra[0].cursor, 7);
        doc.add_cursor_vertical(true);
        assert_eq!(doc.extra[1].cursor, 11);
        doc.move_cursors_vertical(false, false);
        assert_eq!(doc.cursors().iter().map(|e| e.cursor).collect::<Vec<_>>(), vec![3, 2, 7]);
        assert!(doc.collapse_cursors());
        assert!(!doc.has_extra());
    }

    #[test]
    fn cores_so_do_trecho_a_vista() {
        let spans = vec![(0..3, 1), (4..5, 2), (8..9, 3), (12..20, 4)];
        assert_eq!(spans_in(&spans, 4, 9), &spans[1..3]);
        // A que começa antes e entra no trecho vem junto.
        assert_eq!(spans_in(&spans, 14, 30), &spans[3..4]);
        assert!(spans_in(&spans, 21, 30).is_empty());
        // Fora de ordem: todas (quem desenha recorta linha a linha).
        let messy = vec![(4..5, 2), (0..3, 1)];
        assert_eq!(spans_in(&messy, 0, 1).len(), 2);
        assert_eq!(ranges_in(&[0..2, 5..9], 3, 4), &[] as &[Range<usize>]);
    }

    #[test]
    fn linhas_e_posicoes() {
        let text = "um\ndois\n\ntrês";
        let starts = line_starts(text);
        assert_eq!(starts, vec![0, 3, 8, 9]);
        assert_eq!(line_of(&starts, 0), 0);
        assert_eq!(line_of(&starts, 4), 1);
        assert_eq!(line_of(&starts, 8), 2);
        assert_eq!(line_of(&starts, text.len()), 3);
        assert_eq!(&text[starts[1]..line_end(text, &starts, 1)], "dois");
        assert_eq!(line_end(text, &starts, 3), text.len());
    }

    #[test]
    fn coluna_por_largura() {
        let text = "abc\nxyz";
        let starts = line_starts(text);
        // Cada letra com 10 px: x=14 fica entre a 1ª e a 2ª letra, mais perto da 2ª.
        let at = index_in_line(text, &starts, 1, 14.0, |s| s.chars().count() as f32 * 10.0);
        assert_eq!(at, 5);
        assert_eq!(index_in_line(text, &starts, 0, 999.0, |s| s.len() as f32 * 10.0), 3);
    }

    #[test]
    fn coluna_por_largura_em_linha_longa() {
        // Mesmo resultado da busca caractere a caractere, com poucas medidas.
        let text = format!("{}é{}", "a".repeat(3000), "b".repeat(10));
        let starts = line_starts(&text);
        let width = |s: &str| s.chars().count() as f32 * 7.0;
        for x in [0.0, 3.4, 3.6, 7000.0, 21_003.0, 21_004.0, 21_010.0, 99_999.0] {
            let slow = {
                let mut left = 0.0;
                let mut at = text.len();
                for (i, c) in text.char_indices() {
                    let right = width(&text[..i + c.len_utf8()]);
                    if x < (left + right) / 2.0 {
                        at = i;
                        break;
                    }
                    left = right;
                }
                at
            };
            let mut calls = 0;
            assert_eq!(index_in_line(&text, &starts, 0, x, |s| { calls += 1; width(s) }), slow, "x = {x}");
            assert!(calls < 60, "{calls} medidas");
        }
    }

    #[test]
    fn desfazer_junta_a_digitacao_e_refazer_volta() {
        let mut doc = TextDoc::new(String::new());
        doc.type_text("o");
        doc.type_text("l");
        doc.type_text("á");
        doc.type_text(" ");
        doc.type_text("mundo");
        assert_eq!(doc.text, "olá mundo");
        assert!(doc.undo());
        assert_eq!(doc.text, "olá ");
        assert!(doc.undo());
        assert_eq!(doc.text, "olá");
        assert!(doc.undo());
        assert_eq!(doc.text, "");
        assert!(!doc.undo());
        assert!(doc.redo());
        assert_eq!(doc.text, "olá");
        // Mudar o texto depois de desfazer apaga o que dava para refazer.
        doc.type_text("!");
        assert!(!doc.redo());
    }

    #[test]
    fn pedacos_de_cor() {
        let text = "x = 2 + y\nsegunda";
        let starts = line_starts(text);
        let (s, e) = (starts[0], line_end(text, &starts, 0));
        let blue = [0, 0, 255, 255];
        let red = [255, 0, 0, 255];
        // Sem cores: a linha inteira na cor normal; linha vazia, nada.
        assert_eq!(color_runs(text, s, e, &[]), [(0, 9, None)]);
        assert!(color_runs("", 0, 0, &[]).is_empty());
        // Em ordem, com os buracos na cor normal; o trecho que passa da linha é cortado.
        let runs = color_runs(text, s, e, &[(8..12, red), (0..1, blue)]);
        assert_eq!(runs, [(0, 1, Some(blue)), (1, 8, None), (8, 9, Some(red))]);
        // Fora do texto, no meio de um caractere, vazio ou invertido: ignorados.
        let acento = "pé = 1";
        #[allow(clippy::reversed_empty_ranges)]
        let bad = [(0..2, blue), (0..99, red), (3..3, red), (4..2, red)];
        assert_eq!(color_runs(acento, 0, acento.len(), &bad), [(0, acento.len(), None)]);
        // Dois que se cobrem: vale o primeiro da lista.
        assert_eq!(color_runs(text, s, e, &[(0..5, blue), (2..9, red)]), [(0, 5, Some(blue)), (5, 9, Some(red))]);
        // Sublinhados: cortados na linha, os inválidos saem.
        assert_eq!(underline_runs(text, starts[1], text.len(), &[8..13, 0..99]), [(10, 13)]);
    }

    #[test]
    fn tabulacao_corta_o_pedaco_do_meio() {
        let text = "aluguel\t* 2 =\tfim";
        // O pedaço que começa a linha fica inteiro, mesmo com tabulação.
        assert_eq!(tab_pieces(text, 0, 0, 7), [(0, 7)]);
        assert_eq!(tab_pieces(text, 0, 0, text.len()), [(0, text.len())]);
        // Do meio da linha: cortado em cada tabulação, sem ela.
        assert_eq!(tab_pieces(text, 0, 7, text.len()), [(8, 13), (14, 17)]);
        // Tabulações seguidas e no fim: nenhum trecho vazio.
        assert_eq!(tab_pieces("a\t\tb\t", 0, 1, 5), [(3, 4)]);
        // Sem tabulação: o pedaço como veio.
        assert_eq!(tab_pieces(text, 0, 8, 13), [(8, 13)]);
    }

    #[test]
    fn pode_desfazer_e_refazer() {
        let mut doc = TextDoc::new("a".into());
        assert!(!doc.can_undo() && !doc.can_redo());
        doc.type_text("b");
        assert!(doc.can_undo() && !doc.can_redo());
        doc.undo();
        assert!(!doc.can_undo() && doc.can_redo());
    }

    #[test]
    fn nova_linha_mantem_o_recuo() {
        let mut doc = TextDoc::new("fn main() {\n    let x = 1;".into());
        doc.edit = Edit::at_end(&doc.text);
        doc.newline();
        doc.type_text("y");
        assert_eq!(doc.text, "fn main() {\n    let x = 1;\n    y");
        assert!(doc.modified);
    }
}
