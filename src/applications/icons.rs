//! Ícones prontos para desenhar com tiny-skia: os símbolos da interface embutidos no
//! mide-ui ([`SYMBOLS`]), o tema do mide (`mide-icons`) e, só como reserva, o tema do
//! sistema.
//!
//! Ordem da busca (`load_themed_icon`, usado por `ui.icon`, `icon_button`...):
//! 1. símbolo embutido do kit, para os nomes de [`SYMBOLS`] (os mesmos em qualquer
//!    desktop: abrir arquivo é um documento e abrir pasta é uma pasta, mesmo onde o
//!    tema desenha os dois iguais);
//! 2. `MIDE_ICON_THEME` (testes) e o tema do mide, quando instalado;
//! 3. o tema do sistema (gsettings) e o Adwaita, só como reserva;
//! 4. o `hicolor`.

use std::{fs, path::Path};

use tiny_skia::Pixmap;

/// Nome do tema de ícones do mide (o fork do WhiteSur, `mide-icons`, instalado pelo
/// `mide-update`).
pub const MIDE_THEME: &str = "mide";

/// Ícone de app pelo nome: primeiro no tema do mide (quando instalado: os apps de
/// terceiros ganham a versão no estilo dele, como no GTK), depois no tema padrão
/// (`hicolor`, onde os apps instalam os ícones) e nos temas do usuário. Os apps do
/// mide (`mide-terminal`...) caem num ícone genérico sem o tema do mide. Nomes de
/// arquivo (caminho absoluto) também valem.
pub fn load_icon(name: &str, size: u32) -> Option<Pixmap> {
    if name.starts_with('/') {
        return load_icon_file(name, size);
    }
    let in_mide = || {
        let icon = mide_theme_installed().then(|| in_theme(MIDE_THEME, &[name], size)).flatten();
        if icon.is_some() {
            tracing::debug!("ícone {name}: tema do mide");
        }
        icon
    };
    in_mide()
        .or_else(|| load_icon_hicolor(name, size))
        .or_else(|| themed(&[name], size))
        .or_else(|| generic_name(name).and_then(|g| load_icon_hicolor(g, size).or_else(|| themed(&[g], size))))
}

/// Ícone genérico de um app do mide, para quando o tema do mide não está instalado.
pub fn generic_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "mide-terminal" => "utilities-terminal",
        "mide-text" | "mide-code" => "accessories-text-editor",
        "mide-writer" => "x-office-document",
        "mide-files" => "system-file-manager",
        "mide-settings" => "preferences-system",
        "mide-screenshot" | "mide-screenshot-editor" => "applets-screenshooter",
        "mide-calculator" => "accessories-calculator",
        "mide-about" => "computer",
        _ => return None,
    })
}

/// O tema do mide está instalado (em `~/.local/share/icons` ou nas pastas de dados
/// do sistema)?
pub fn mide_theme_installed() -> bool {
    static INSTALLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *INSTALLED.get_or_init(|| {
        let home = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")));
        let system = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
        home.into_iter()
            .chain(system.split(':').filter(|d| !d.is_empty()).map(std::path::PathBuf::from))
            .any(|dir| dir.join("icons").join(MIDE_THEME).join("index.theme").is_file())
    })
}

/// Ícone do tema padrão: PNG direto, SVG renderizado.
fn load_icon_hicolor(name: &str, size: u32) -> Option<Pixmap> {
    let path = freedesktop_icons::lookup(name)
        .with_size(size.min(u16::MAX as u32) as u16)
        .with_cache()
        .find()?;
    match path.extension().and_then(|e| e.to_str()) {
        Some("png") => icon_from_png(&path, size),
        Some("svg") | Some("svgz") => icon_from_svg(&path, size),
        _ => None,
    }
}

/// Ícone de um arquivo (PNG, JPEG ou SVG), ajustado a `size`. Aceita `file://`.
pub fn load_icon_file(path: &str, size: u32) -> Option<Pixmap> {
    let path = Path::new(path.strip_prefix("file://").unwrap_or(path));
    // Só arquivos comuns e de tamanho razoável (o `Icon=` de um atalho pode apontar
    // para um FIFO, que travaria a janela, ou para um arquivo enorme).
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= 32 << 20) {
        return None;
    }
    match path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref() {
        Some("svg") | Some("svgz") => icon_from_svg(path, size),
        Some(_) => icon_from_png(path, size),
        None => None,
    }
}

/// Temas de ícones do usuário, em ordem: `MIDE_ICON_THEME`, o tema do mide (se
/// instalado), o tema do GNOME (gsettings) e o Adwaita.
fn user_themes() -> &'static [String] {
    static THEMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    THEMES.get_or_init(|| {
        let mut themes = Vec::new();
        if let Ok(theme) = std::env::var("MIDE_ICON_THEME") {
            themes.push(theme);
        }
        if mide_theme_installed() {
            themes.push(MIDE_THEME.into());
        }
        let gnome = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "icon-theme"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().trim_matches('\'').to_string())
            .filter(|t| !t.is_empty());
        themes.extend(gnome);
        themes.push("Adwaita".into());
        themes.dedup();
        themes
    })
}

/// Ícone de lugar ou de tipo de arquivo (pasta, lixeira, imagem...), procurado no
/// tema do usuário: esses ícones não ficam no tema padrão `hicolor`. Tenta cada
/// nome em ordem e devolve o primeiro que existir.
pub fn load_themed_icon(names: &[&str], size: u32) -> Option<Pixmap> {
    // Caminho de arquivo (ícone de um atalho `.desktop`, E8): o próprio arquivo.
    if let Some(file) = names.first().filter(|n| n.starts_with('/')) {
        return load_icon_file(file, size).or_else(|| load_themed_icon(&names[1..], size));
    }
    if let Some(icon) = names.iter().find_map(|n| symbol(n, size)) {
        return Some(icon);
    }
    themed(names, size).or_else(|| names.iter().find_map(|n| load_icon_hicolor(n, size)))
}

/// Primeiro dos nomes encontrado nos temas do usuário.
fn themed(names: &[&str], size: u32) -> Option<Pixmap> {
    user_themes().iter().find_map(|theme| in_theme(theme, names, size))
}

/// Primeiro dos nomes encontrado em `theme` (sem cair no `hicolor`).
fn in_theme(theme: &str, names: &[&str], size: u32) -> Option<Pixmap> {
    for name in names {
        // Nos tamanhos pequenos (16 a 24), temas como o do mide (do WhiteSur) trazem
        // desenhos só de contorno na cor do texto: a pasta da lista da dock parecia um
        // símbolo. Um ícone colorido vem do desenho grande, reduzido para `size`; só os
        // "-symbolic" são de contorno de propósito.
        let lookup = if name.ends_with("-symbolic") { size } else { size.max(48) };
        let path = freedesktop_icons::lookup(name)
            .with_size(lookup.min(u16::MAX as u32) as u16)
            .with_theme(theme)
            .with_cache()
            .find();
        if let Some(path) = path {
            // O `hicolor` entra na busca de qualquer tema como herança: aqui só vale o
            // do próprio tema.
            if path.components().any(|c| c.as_os_str() == "hicolor") && theme != "hicolor" {
                continue;
            }
            let icon = match path.extension().and_then(|e| e.to_str()) {
                Some("png") => icon_from_png(&path, size),
                Some("svg") | Some("svgz") => icon_from_svg(&path, size),
                _ => None,
            };
            if icon.is_some() {
                return icon;
            }
        }
    }
    None
}

/// Retângulo (x, y, largura, altura) dos pixels visíveis de uma imagem RGBA.
/// Ícones de tema costumam ter margem transparente de tamanhos diferentes; sem
/// cortá-la, cada ícone parece de um tamanho e os espaços em volta ficam desiguais.
fn visible_bounds(rgba: &[u8], width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (width, height, 0u32, 0u32);
    for y in 0..height {
        for x in 0..width {
            if rgba[((y * width + x) * 4 + 3) as usize] > 8 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0 && y1 > y0).then(|| (x0, y0, x1 - x0, y1 - y0))
}

/// PNG com a parte visível ajustada ao tamanho do ícone, centralizada.
fn icon_from_png(path: &Path, size: u32) -> Option<Pixmap> {
    let img = image::open(path).ok()?.to_rgba8();
    let (bx, by, bw, bh) = visible_bounds(img.as_raw(), img.width(), img.height())?;
    let visible = image::imageops::crop_imm(&img, bx, by, bw, bh).to_image();
    let k = size as f32 / bw.max(bh) as f32;
    let (w, h) = (((bw as f32 * k).round() as u32).max(1), ((bh as f32 * k).round() as u32).max(1));
    let resized = image::imageops::resize(&visible, w, h, image::imageops::FilterType::Lanczos3);
    let (ox, oy) = ((size - w) / 2, (size - h) / 2);
    let mut pixmap = Pixmap::new(size, size)?;
    let data = pixmap.data_mut();
    for (x, y, px) in resized.enumerate_pixels() {
        let i = (((y + oy) * size + x + ox) * 4) as usize;
        // tiny-skia guarda pixels premultiplicados.
        let a = px[3] as u32;
        data[i] = (px[0] as u32 * a / 255) as u8;
        data[i + 1] = (px[1] as u32 * a / 255) as u8;
        data[i + 2] = (px[2] as u32 * a / 255) as u8;
        data[i + 3] = px[3];
    }
    Some(pixmap)
}

/// Símbolos da interface desenhados no próprio mide-ui (desenho nosso, traço de 1,3
/// num quadro de 16): usados no lugar do tema para que os apps do mide tenham sempre os
/// mesmos símbolos. Ficam pretos; os componentes pintam na cor do texto (`-symbolic`).
pub const SYMBOLS: &[(&str, &str)] = &[
    // Abrir arquivo: um documento (com a dobra no canto e duas linhas de texto).
    ("document-open-symbolic", r##"<path d="M3.5 1.5h6l3 3v10h-9z" fill="none" stroke="#000" stroke-width="1.3" stroke-linejoin="round"/><path d="M9.5 1.5v3h3" fill="none" stroke="#000" stroke-width="1.3" stroke-linejoin="round"/><path d="M5.8 8.5h4.4M5.8 11h4.4" stroke="#000" stroke-width="1.2" stroke-linecap="round"/>"##),
    // Novo documento: o documento com um "+".
    ("document-new-symbolic", r##"<path d="M3.5 1.5h6l3 3v10h-9z" fill="none" stroke="#000" stroke-width="1.3" stroke-linejoin="round"/><path d="M9.5 1.5v3h3" fill="none" stroke="#000" stroke-width="1.3" stroke-linejoin="round"/><path d="M8 7.2v5M5.5 9.7h5" stroke="#000" stroke-width="1.3" stroke-linecap="round"/>"##),
    // Salvar: seta descendo para a bandeja.
    ("document-save-symbolic", r##"<path d="M8 1.5v8.5M4.6 6.8 8 10.2l3.4-3.4" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/><path d="M1.8 10.2v2.8a1.5 1.5 0 0 0 1.5 1.5h9.4a1.5 1.5 0 0 0 1.5-1.5v-2.8" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>"##),
    // Abrir pasta: a pasta.
    ("folder-open-symbolic", r##"<path d="M1.5 4a1.2 1.2 0 0 1 1.2-1.2h3.4l1.6 1.6h5.6a1.2 1.2 0 0 1 1.2 1.2v6.9a1.2 1.2 0 0 1-1.2 1.2H2.7a1.2 1.2 0 0 1-1.2-1.2z" fill="none" stroke="#000" stroke-width="1.3" stroke-linejoin="round"/><path d="M1.5 6.6h13" stroke="#000" stroke-width="1.2"/>"##),
    // Buscar: lupa.
    ("edit-find-symbolic", r##"<circle cx="6.6" cy="6.6" r="4.6" fill="none" stroke="#000" stroke-width="1.5"/><path d="m10 10 4.3 4.3" stroke="#000" stroke-width="1.7" stroke-linecap="round"/>"##),
    // Preferências: engrenagem de oito dentes.
    ("preferences-system-symbolic", r##"<circle cx="8" cy="8" r="5.6" fill="none" stroke="#000" stroke-width="2.6" stroke-dasharray="2.2 2.2"/><circle cx="8" cy="8" r="3.7" fill="none" stroke="#000" stroke-width="2"/>"##),
    // Mostrar a barra lateral: janela com a coluna da esquerda.
    ("view-sidebar-start-symbolic", r##"<rect x="1.5" y="2.5" width="13" height="11" rx="2.2" fill="none" stroke="#000" stroke-width="1.3"/><path d="M6 2.5v11" stroke="#000" stroke-width="1.3"/><path d="M3.3 5.2h1.2M3.3 7.2h1.2M3.3 9.2h1.2" stroke="#000" stroke-width="1.1" stroke-linecap="round"/>"##),
    // Mostrar o painel de baixo (terminal integrado).
    ("view-dual-symbolic", r##"<rect x="1.5" y="2.5" width="13" height="11" rx="2.2" fill="none" stroke="#000" stroke-width="1.3"/><path d="M1.5 9.5h13" stroke="#000" stroke-width="1.3"/>"##),
    // Terminal: janela com o prompt.
    ("utilities-terminal-symbolic", r##"<rect x="1.5" y="2.5" width="13" height="11" rx="2.2" fill="none" stroke="#000" stroke-width="1.3"/><path d="m4.3 6.2 2.2 1.8-2.2 1.8M8 10.6h3.5" fill="none" stroke="#000" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"##),
    // Adicionar.
    ("list-add-symbolic", r##"<path d="M8 2.5v11M2.5 8h11" stroke="#000" stroke-width="1.5" stroke-linecap="round"/>"##),
    // Fechar.
    ("window-close-symbolic", r##"<path d="m3.5 3.5 9 9m0-9-9 9" stroke="#000" stroke-width="1.5" stroke-linecap="round"/>"##),
    // Desligar: o círculo aberto em cima com o traço.
    ("system-shutdown-symbolic", r##"<path d="M8 1.6v5.6" stroke="#000" stroke-width="1.5" stroke-linecap="round"/><path d="M4.7 3.9a5.3 5.3 0 1 0 6.6 0" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round"/>"##),
    // Reiniciar: a volta com a ponta da seta.
    ("system-reboot-symbolic", r##"<path d="M13.2 8.4A5.2 5.2 0 1 1 11.6 4.3" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round"/><path d="M12.2 1.4v3.4H8.8" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"##),
    // Repousar: a lua.
    ("system-suspend-symbolic", r##"<path d="M13.6 9.8A6 6 0 0 1 6.2 2.4a6 6 0 1 0 7.4 7.4z" fill="none" stroke="#000" stroke-width="1.4" stroke-linejoin="round"/>"##),
    // Seguir (enviar a senha) e voltar.
    ("go-next-symbolic", r##"<path d="M2.5 8h10.6M9 3.8 13.2 8 9 12.2" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"##),
    ("go-previous-symbolic", r##"<path d="M13.5 8H2.9M7 3.8 2.8 8 7 12.2" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"##),
    // Caps Lock ligado: a seta oca com a barra embaixo.
    ("caps-lock-symbolic", r##"<path d="M8 1.6 2.4 7.6h3.1v3.2h5V7.6h3.1z" fill="none" stroke="#000" stroke-width="1.3" stroke-linejoin="round"/><path d="M5.5 13.6h5" stroke="#000" stroke-width="1.4" stroke-linecap="round"/>"##),
    // Pessoa (outro usuário, foto que falta).
    ("avatar-default-symbolic", r##"<circle cx="8" cy="4.6" r="3.1" fill="#000"/><path d="M1.8 15.2c0-3.6 2.8-6 6.2-6s6.2 2.4 6.2 6z" fill="#000"/>"##),
];

/// Símbolo embutido pelo nome (ver [`SYMBOLS`]), no tamanho pedido.
pub fn symbol(name: &str, size: u32) -> Option<Pixmap> {
    let (_, body) = SYMBOLS.iter().find(|(n, _)| *n == name)?;
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">{body}</svg>"#);
    svg_to_icon(svg.as_bytes(), size)
}

/// Renderiza o SVG com a parte visível ajustada ao tamanho do ícone. Primeiro
/// renderiza grande para achar a parte visível, depois renderiza de novo direto no
/// tamanho final (fica nítido). O resvg usa a própria cópia do tiny-skia; só os
/// bytes premultiplicados são copiados, então as versões não precisam bater.
fn icon_from_svg(path: &Path, size: u32) -> Option<Pixmap> {
    svg_to_icon(&fs::read(path).ok()?, size)
}

/// Como `icon_from_svg`, a partir dos bytes do SVG.
fn svg_to_icon(data: &[u8], size: u32) -> Option<Pixmap> {
    // Grande o bastante para medir a margem com precisão, sem pesar.
    let probe_size = size * 2;
    let tree = resvg::usvg::Tree::from_data(data, &resvg::usvg::Options::default()).ok()?;
    let svg_size = tree.size();
    let s0 = probe_size as f32 / svg_size.width().max(svg_size.height());
    let mut probe = resvg::tiny_skia::Pixmap::new(probe_size, probe_size)?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s0, s0), &mut probe.as_mut());
    let (bx, by, bw, bh) = visible_bounds(probe.data(), probe_size, probe_size)?;

    let k = size as f32 / bw.max(bh) as f32;
    let ox = (size as f32 - bw as f32 * k) / 2.0 - bx as f32 * k;
    let oy = (size as f32 - bh as f32 * k) / 2.0 - by as f32 * k;
    let mut rendered = resvg::tiny_skia::Pixmap::new(size, size)?;
    let transform = resvg::tiny_skia::Transform::from_translate(ox, oy).pre_scale(s0 * k, s0 * k);
    resvg::render(&tree, transform, &mut rendered.as_mut());
    let mut pixmap = Pixmap::new(size, size)?;
    pixmap.data_mut().copy_from_slice(rendered.data());
    Some(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simbolos_embutidos_desenham() {
        for (name, _) in SYMBOLS {
            let icon = symbol(name, 16).unwrap_or_else(|| panic!("{name} não desenhou"));
            assert!(icon.data().chunks(4).any(|p| p[3] > 0), "{name} vazio");
        }
        // Abrir arquivo e abrir pasta são desenhos diferentes (o pedido do E11).
        assert_ne!(symbol("document-open-symbolic", 16).unwrap().data(), symbol("folder-open-symbolic", 16).unwrap().data());
        // O símbolo embutido vale antes de qualquer tema.
        assert_eq!(load_themed_icon(&["document-open-symbolic"], 16).unwrap().data(), symbol("document-open-symbolic", 16).unwrap().data());
    }

    /// Bancada: `cargo test --lib bancada_icones -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bancada_icones() {
        for (names, size) in [(&["user-home-symbolic"][..], 18), (&["folder", "text-x-generic"][..], 18), (&["folder", "text-x-generic"][..], 64), (&["text-x-generic"][..], 18), (&["application-pdf", "text-x-generic"][..], 18), (&["folder-download-symbolic"][..], 18)] {
            let t = std::time::Instant::now();
            let found = load_themed_icon(names, size).is_some();
            let first = t.elapsed();
            let t = std::time::Instant::now();
            load_themed_icon(names, size);
            println!("{:?}@{size}: {found} primeira {:.1} ms, de novo {:.1} ms", names[0], first.as_secs_f32() * 1e3, t.elapsed().as_secs_f32() * 1e3);
        }
    }
}
