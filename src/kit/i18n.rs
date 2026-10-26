//! Ponto de entrada da tradução da interface (item I1 do mide).
//!
//! Hoje [`tr`] devolve o próprio texto: os apps ainda escrevem os textos direto.
//! Quando o sistema de tradução chegar (catálogos por idioma, com o inglês como base e
//! o pt-BR como primeira tradução), ele entra aqui, e todo texto que já passa por
//! `tr` (as dicas dos botões de ícone, por exemplo) passa a ser traduzido sem mudar os
//! apps. Textos novos da interface devem passar por `tr`.

use std::borrow::Cow;

/// Texto da interface no idioma da sessão (por enquanto, o próprio texto).
pub fn tr(text: &str) -> Cow<'_, str> {
    Cow::Borrowed(text)
}
