//! Tema do mide-ui: cores, cantos e tamanhos num lugar só. Os componentes leem tudo
//! daqui, então mudar o tema muda todos os apps feitos com o mide-ui.

use tiny_skia::Color;

/// Medidas do Tahoe (`docs/tahoe.md`, seção 2).
/// Janela com barra lateral ou barra de ferramentas.
pub const RADIUS_WINDOW: f32 = 24.0;
/// Recuo dos painéis flutuantes (barra lateral) até a borda da janela.
pub const INSET_FLOAT: f32 = 8.0;
/// Barra lateral flutuante (raio da janela menos o recuo).
pub const RADIUS_SIDEBAR: f32 = RADIUS_WINDOW - INSET_FLOAT;
/// Seleção dentro de menus e da barra lateral, linhas de lista.
pub const RADIUS_ITEM: f32 = 8.0;
/// Barra de ferramentas e as cápsulas dela.
pub const TOOLBAR_HEIGHT: f32 = 46.0;
pub const TOOLBAR_ITEM: f32 = 30.0;
/// Largura de cada botão de um grupo da barra de ferramentas (`tool_group`).
pub const TOOLBAR_SEG: f32 = 32.0;

/// Cor em RGBA (0 a 255).
pub type Rgba = [u8; 4];

/// Aparência dos componentes.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Fundo da janela.
    pub background: Rgba,
    /// Fundo da barra lateral.
    pub sidebar: Rgba,
    /// Fundo dos cartões.
    pub card: Rgba,
    /// Linhas finas (bordas e separadores).
    pub line: Rgba,
    /// Texto principal e secundário.
    pub text: Rgba,
    pub text_secondary: Rgba,
    /// Nome embaixo dos ladrilhos (`tile`). Claro (sobre o papel de parede), ganha uma
    /// sombra para ser lido em qualquer fundo.
    pub tile_text: Rgba,
    /// Cor de destaque (botão principal, interruptor ligado, seleção).
    pub accent: Rgba,
    /// Texto sobre a cor de destaque.
    pub on_accent: Rgba,
    /// Fundo de botões comuns e de itens sob o ponteiro.
    pub control: Rgba,
    pub hover: Rgba,
    /// Base dos tons de contraste (trilhos, bordas de campos, listras, barras de
    /// rolagem): preto no tema claro, branco no escuro; a transparência vem de quem usa
    /// (`Theme::shade`).
    pub ink: Rgba,
    /// Fundo dos menus de contexto e listas suspensas.
    pub menu: Rgba,
    /// Ações destrutivas (Apagar, Mover para a Lixeira).
    pub danger: Rgba,
    /// Barra lateral sobre o vidro (o desfoque aparece através dela).
    pub sidebar_glass: Rgba,
    /// Tema escuro (apps que desenham por conta própria escolhem as cores por aqui).
    pub dark: bool,
    /// Cor que pinta a interface (Configurações → Aparência), ou `None` no padrão e nas
    /// cores neutras. Quem pinta fundos com cor fixa (menus, popovers, vidro) passa por
    /// [`Theme::tinted`].
    pub tint: Option<[u8; 3]>,
    /// Cantos dos cartões e dos controles.
    pub radius: f32,
    pub control_radius: f32,
    /// Tamanhos de fonte.
    pub font: f32,
    pub font_small: f32,
    pub font_title: f32,
    /// Altura de uma linha de controle (botão, campo, item de lista).
    pub row: f32,
    /// Espaço entre itens empilhados e margem das bordas.
    pub spacing: f32,
    pub padding: f32,
}

impl Default for Theme {
    /// Tema claro do mide, o mesmo das Configurações do Sistema.
    fn default() -> Self {
        Self {
            background: [246, 246, 248, 255],
            sidebar: [232, 232, 236, 255],
            card: [255, 255, 255, 255],
            line: [0, 0, 0, 24],
            text: [24, 24, 28, 255],
            text_secondary: [130, 130, 138, 255],
            tile_text: [24, 24, 28, 255],
            accent: [10, 110, 235, 255],
            on_accent: [255, 255, 255, 255],
            control: [0, 0, 0, 16],
            hover: [0, 0, 0, 28],
            ink: [0, 0, 0, 255],
            menu: [246, 246, 248, 245],
            danger: [210, 40, 40, 255],
            sidebar_glass: [248, 248, 250, 178],
            dark: false,
            tint: None,
            radius: 10.0,
            control_radius: 7.0,
            font: 13.5,
            font_small: 12.0,
            font_title: 20.0,
            row: 32.0,
            spacing: 10.0,
            padding: 24.0,
        }
    }
}

impl Theme {
    /// Tema escuro do mide: fundos grafite, texto claro e o mesmo azul de destaque
    /// (um pouco mais claro, para ler bem sobre o escuro).
    pub fn dark() -> Self {
        Self {
            background: [30, 30, 32, 255],
            sidebar: [40, 40, 44, 255],
            card: [44, 44, 48, 255],
            line: [255, 255, 255, 26],
            text: [236, 236, 240, 255],
            text_secondary: [152, 152, 160, 255],
            tile_text: [236, 236, 240, 255],
            accent: [30, 125, 245, 255],
            on_accent: [255, 255, 255, 255],
            control: [255, 255, 255, 22],
            hover: [255, 255, 255, 34],
            ink: [255, 255, 255, 255],
            menu: [44, 44, 48, 245],
            danger: [255, 95, 85, 255],
            sidebar_glass: [36, 36, 40, 178],
            dark: true,
            ..Self::default()
        }
    }

    /// Tema escolhido em Configurações → Aparência (`[appearance] theme` no
    /// `config.toml`: "light" ou "dark"), com a cor de destaque escolhida
    /// (`[appearance] accent`).
    pub fn system() -> Self {
        let dark = system_dark();
        let mut theme = if dark { Self::dark() } else { Self::default() };
        theme.accent = system_accent(dark);
        theme.on_accent = on_color(theme.accent);
        // Interface na cor escolhida (como os Macs coloridos): fundos, barras laterais,
        // cartões e menus puxam de leve para a cor. O padrão (azul) e as cores neutras
        // (grafite, branco, preto) deixam o claro e o escuro como sempre foram.
        theme.tint = ui_tint(dark);
        if let Some(c) = theme.tint {
            // No claro a mesma mistura aparece menos (fundos quase brancos): um pouco mais.
            let (bg, side, card, menu, glass) = if dark { (0.12, 0.16, 0.09, 0.14, 0.20) } else { (0.16, 0.24, 0.07, 0.16, 0.30) };
            theme.background = mix(theme.background, c, bg);
            theme.sidebar = mix(theme.sidebar, c, side);
            theme.card = mix(theme.card, c, card);
            theme.menu = mix(theme.menu, c, menu);
            theme.sidebar_glass = mix(theme.sidebar_glass, c, glass);
        }
        theme
    }

    /// Fundo `base` (menus, popovers, painéis de vidro) com o toque da cor escolhida,
    /// mantendo a transparência; igual a `base` no padrão.
    pub fn tinted(&self, base: Rgba) -> Rgba {
        match self.tint {
            Some(c) => mix(base, c, if self.dark { 0.25 } else { 0.34 }),
            None => base,
        }
    }

    /// Tom de contraste com a transparência `alpha` (preto no claro, branco no escuro).
    pub fn shade(&self, alpha: u8) -> Rgba {
        [self.ink[0], self.ink[1], self.ink[2], alpha]
    }
}

/// Cores de destaque (Configurações → Aparência), como as do Mac: chave gravada, nome
/// na tela (em inglês; quem mostra passa por [`tr`](crate::kit::tr)) e a cor no tema claro e no escuro (um pouco mais clara no escuro, para ler
/// bem). O texto por cima de cada uma vem de [`on_color`]: branco nas cores, escuro no
/// branco. O branco no tema claro e o preto no escuro ficam um tom afastados do fundo,
/// para a seleção e os interruptores ligados continuarem à vista.
pub const ACCENTS: &[(&str, &str, [u8; 3], [u8; 3])] = &[
    ("blue", "Default (Blue)", [10, 110, 235], [30, 125, 245]),
    ("purple", "Purple", [135, 70, 200], [160, 95, 225]),
    ("pink", "Pink", [215, 50, 125], [235, 80, 150]),
    ("red", "Red", [215, 45, 45], [235, 75, 70]),
    ("orange", "Orange", [225, 110, 10], [240, 130, 35]),
    ("yellow", "Yellow", [195, 140, 0], [215, 160, 10]),
    ("green", "Green", [35, 150, 65], [50, 170, 80]),
    ("graphite", "Graphite", [115, 115, 125], [140, 140, 150]),
    ("white", "White", [222, 222, 228], [242, 242, 246]),
    ("black", "Black", [28, 28, 32], [6, 6, 8]),
];

/// Cor do texto e dos símbolos por cima de `c`: escuro sobre cores claras (o destaque
/// branco), branco nas outras.
pub fn on_color(c: Rgba) -> Rgba {
    let luma = 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
    if luma > 165.0 { [24, 24, 28, 255] } else { [255, 255, 255, 255] }
}

/// Interface na cor de destaque (`[appearance] tint`, ligado por padrão;
/// Configurações → Aparência). Só no mide: fora dele, o tema segue o sistema.
pub fn tint_enabled() -> bool {
    crate::control_client::on_mide() && !matches!(crate::settings_file::cached("appearance", "tint"), Some(crate::settings_file::Value::Bool(false)))
}

/// Cores de destaque que não pintam a interface: a padrão (azul, o visual de sempre) e
/// as neutras.
const NEUTRAL_ACCENTS: &[&str] = &["blue", "graphite", "white", "black"];

/// Cor que pinta a interface (fundos das janelas, barras laterais, menus, vidro), ou
/// `None` com a cor padrão, uma neutra ou o interruptor desligado.
pub fn ui_tint(dark: bool) -> Option<[u8; 3]> {
    if !tint_enabled() || NEUTRAL_ACCENTS.contains(&accent_key().as_str()) {
        return None;
    }
    let [r, g, b, _] = system_accent(dark);
    Some([r, g, b])
}

/// Cor e força do toque de cor no vidro (dock, menus, barra, barras laterais), quando
/// a interface é pintada ([`ui_tint`]). Mais forte no escuro, onde o mesmo tanto de
/// cor aparece menos.
pub fn glass_tint(dark: bool) -> Option<([u8; 3], f32)> {
    ui_tint(dark).map(|c| (c, if dark { 0.18 } else { 0.28 }))
}

/// Transparência do véu na cor escolhida por cima do vidro das peças do shell (dock),
/// como o vidro colorido dos Macs: a cor aparece e o desfoque continua visível através
/// dela. Mais forte no claro, onde o vidro é quase branco.
pub fn tint_veil(dark: bool) -> u8 {
    if dark { 52 } else { 72 }
}

/// `base` com `amount` (0 a 1) de `over`, guardando a transparência de `base`.
pub fn mix(base: Rgba, over: [u8; 3], amount: f32) -> Rgba {
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount.clamp(0.0, 1.0)).round() as u8;
    [m(base[0], over[0]), m(base[1], over[1]), m(base[2], over[2]), base[3]]
}

/// Chave da cor de destaque escolhida (`blue` se nenhuma ou desconhecida).
pub fn accent_key() -> String {
    match crate::settings_file::cached("appearance", "accent") {
        Some(crate::settings_file::Value::Text(k)) if ACCENTS.iter().any(|(key, ..)| *key == k) => k,
        _ => "blue".into(),
    }
}

/// A cor de destaque da sessão no tema claro ou escuro. Fora do mide, a do sistema
/// (portal do freedesktop, `accent-color`), se ele tiver uma.
pub fn system_accent(dark: bool) -> Rgba {
    if !crate::control_client::on_mide() {
        if let Some(c) = portal_appearance().1 {
            return [c[0], c[1], c[2], 255];
        }
    }
    let key = accent_key();
    let (_, _, light, dim) = ACCENTS.iter().find(|(k, ..)| *k == key).copied().unwrap_or(ACCENTS[0]);
    let c = if dark { dim } else { light };
    [c[0], c[1], c[2], 255]
}

/// Barras de rolagem sempre à mostra (`[appearance] scrollbars = "always"`, como a
/// opção de acessibilidade do macOS). O padrão (`"auto"`) mostra a barra só ao rolar
/// ou com o ponteiro perto dela. Em nenhum dos dois ela reserva espaço no layout.
pub fn scrollbars_always() -> bool {
    matches!(crate::settings_file::cached("appearance", "scrollbars"), Some(crate::settings_file::Value::Text(s)) if s == "always")
}

/// O tema da sessão é o escuro. No mide, a escolha das Configurações; fora dele, a do
/// sistema (portal do freedesktop, `color-scheme`: GNOME, KDE e outros).
pub fn system_dark() -> bool {
    if !crate::control_client::on_mide() {
        return portal_appearance().0;
    }
    matches!(crate::settings_file::cached("appearance", "theme"), Some(crate::settings_file::Value::Text(s)) if s == "dark")
}

/// Aparência pedida pelo sistema fora do mide, pelo portal do freedesktop
/// (`org.freedesktop.appearance`): escuro e a cor de destaque. Lida no máximo a cada
/// 2 s (os apps perguntam o tempo todo); sem portal, claro e sem cor.
fn portal_appearance() -> (bool, Option<[u8; 3]>) {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };
    type Cache = Option<(std::time::Instant, (bool, Option<[u8; 3]>))>;
    static CACHE: Mutex<Cache> = Mutex::new(None);
    static READING: AtomicBool = AtomicBool::new(false);
    let cached = CACHE.lock().ok().and_then(|c| *c);
    // Velha ou nunca lida: lê numa thread (o portal pode demorar ou nem existir; a
    // interface nunca espera). Até chegar, vale o último valor (claro na primeira vez).
    if cached.is_none_or(|(at, _)| at.elapsed() > std::time::Duration::from_secs(2)) && !READING.swap(true, Ordering::AcqRel) {
        std::thread::spawn(|| {
            let value = read_portal_appearance().unwrap_or((false, None));
            if let Ok(mut cache) = CACHE.lock() {
                *cache = Some((std::time::Instant::now(), value));
            }
            READING.store(false, Ordering::Release);
        });
    }
    cached.map(|(_, v)| v).unwrap_or((false, None))
}

fn read_portal_appearance() -> Option<(bool, Option<[u8; 3]>)> {
    static CONN: std::sync::OnceLock<Option<zbus::blocking::Connection>> = std::sync::OnceLock::new();
    let conn = CONN.get_or_init(|| zbus::blocking::Connection::session().ok()).as_ref()?;
    let read = |key: &str| -> Option<zbus::zvariant::OwnedValue> {
        let reply = conn
            .call_method(Some("org.freedesktop.portal.Desktop"), "/org/freedesktop/portal/desktop", Some("org.freedesktop.portal.Settings"), "ReadOne", &("org.freedesktop.appearance", key))
            .ok()?;
        reply.body().deserialize::<zbus::zvariant::OwnedValue>().ok()
    };
    let dark = read("color-scheme").and_then(|v| u32::try_from(unwrap_variant(&v)).ok()).is_some_and(|n| n == 1);
    let accent = read("accent-color").and_then(|v| {
        let s = zbus::zvariant::Structure::try_from(unwrap_variant(&v)).ok()?;
        let f: Vec<f64> = s.fields().iter().filter_map(|x| f64::try_from(x).ok()).collect();
        // Fora de 0..1: o sistema não escolheu cor.
        (f.len() == 3 && f.iter().all(|c| (0.0..=1.0).contains(c))).then(|| [(f[0] * 255.0) as u8, (f[1] * 255.0) as u8, (f[2] * 255.0) as u8])
    });
    Some((dark, accent))
}

/// O portal às vezes manda o valor dentro de outro variant.
fn unwrap_variant(v: &zbus::zvariant::OwnedValue) -> zbus::zvariant::Value<'static> {
    let mut value: zbus::zvariant::Value<'static> = v.try_clone().map(Into::into).unwrap_or(zbus::zvariant::Value::from(0u32));
    while let zbus::zvariant::Value::Value(inner) = value {
        value = *inner;
    }
    value
}

/// Converte para a cor do tiny-skia.
pub fn color(c: Rgba) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], c[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texto_sobre_o_destaque_contrasta() {
        for (key, _, light, dark) in ACCENTS {
            for c in [light, dark] {
                let on = on_color([c[0], c[1], c[2], 255]);
                let luma = |c: [u8; 3]| 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
                let diff = (luma(*c) - luma([on[0], on[1], on[2]])).abs();
                assert!(diff > 90.0, "{key}: texto sem contraste ({diff})");
            }
        }
    }

    #[test]
    fn mistura_guarda_a_transparencia() {
        assert_eq!(mix([0, 0, 0, 100], [200, 100, 50], 0.5), [100, 50, 25, 100]);
        assert_eq!(mix([10, 20, 30, 255], [200, 100, 50], 0.0), [10, 20, 30, 255]);
    }
}
