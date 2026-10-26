//! Janela filha, barra lateral de vidro com divisória, painel de baixo e botões de
//! ícone com dica: o esqueleto de um editor (como o Code do mide).
//!
//! - A engrenagem abre "Preferências" numa janela própria, de vidro, que pertence à
//!   principal (`ui.open_window` + `App::window_view`).
//! - A barra lateral (`ui.sidebar_panel`) e o painel de baixo (`ui.split_vertical`) têm
//!   divisórias arrastáveis; as medidas ficam guardadas no app.
//! - `MIDE_UI_EXEMPLO_PREFS=1` abre as Preferências logo no começo (testes).

use mide_ui::kit::{run, App, Tool, Ui, Window};

struct Editor {
    largura_lateral: f32,
    altura_terminal: f32,
    terminal: bool,
    texto: String,
    fonte: usize,
    numeros: bool,
    abriu_prefs: bool,
    aviso: String,
    /// Caracteres no campo no último quadro (o log avisa quando muda: testes).
    contado: usize,
}

/// Devolve a contagem anterior e guarda a nova.
fn antes_total(contado: &mut usize, agora: usize) -> usize {
    std::mem::replace(contado, agora)
}

impl App for Editor {
    fn view(&mut self, ui: &mut Ui) {
        if !self.abriu_prefs && std::env::var_os("MIDE_UI_EXEMPLO_PREFS").is_some() {
            self.abriu_prefs = true;
            ui.open_window("prefs", prefs_window());
        }
        let r = ui.sidebar_panel(
            &mut self.largura_lateral,
            160.0,
            420.0,
            |ui| {
                ui.at(14.0, 4.0, ui.available_width() - 28.0, |ui| {
                    ui.strong("projeto");
                    for nome in ["Cargo.toml", "src", "main.rs", "README.md"] {
                        ui.list_item(nome, "");
                    }
                });
            },
            |ui| {
                ui.toolbar(|ui| {
                    if let Some(i) = ui.tool_group(&[Tool::new("document-open-symbolic", "Abrir arquivo"), Tool::new("folder-open-symbolic", "Abrir pasta"), Tool::new("document-save-symbolic", "Salvar")]) {
                        self.aviso = format!("botão {i}");
                    }
                    if ui.tool_group(&[Tool::new("utilities-terminal-symbolic", "Terminal").on(self.terminal)]).is_some() {
                        self.terminal = !self.terminal;
                    }
                    if ui.tool_button("preferences-system-symbolic", "Preferências").clicked() {
                        ui.open_window("prefs", prefs_window());
                    }
                });
                let terminal = self.terminal;
                let texto = &mut self.texto;
                let aviso = &self.aviso;
                let mut editor = |ui: &mut Ui| {
                    ui.page(|ui| {
                        ui.text_input(texto, "Digite aqui");
                        ui.note(&format!("{} caractere(s). {aviso}", texto.chars().count()));
                    });
                };
                if terminal {
                    ui.split_vertical(&mut self.altura_terminal, 80.0, 400.0, editor, |ui| {
                        ui.page(|ui| ui.label("$ terminal integrado"));
                    });
                } else {
                    editor(ui);
                }
            },
        );
        let agora = self.texto.chars().count();
        if agora != antes_total(&mut self.contado, agora) {
            tracing::info!("texto com {agora} caractere(s)");
        }
        if r.submitted() {
            // A hora de gravar a largura nas preferências do app.
            self.aviso = format!("barra lateral com {:.0}", self.largura_lateral);
            tracing::info!("{}", self.aviso);
        }
    }

    fn window_view(&mut self, id: &str, ui: &mut Ui) {
        if id != "prefs" {
            return;
        }
        ui.page(|ui| {
            ui.title("Preferências");
            ui.card(|ui| {
                ui.dropdown("Tamanho da fonte", &["12".into(), "13".into(), "14".into()], &mut self.fonte);
                ui.switch("Números de linha", &mut self.numeros);
            });
            if ui.primary_button("Fechar").clicked() {
                ui.close();
            }
        });
    }

    fn window_closed(&mut self, id: &str) {
        tracing::info!("janela {id} fechada pelo app");
        self.aviso = format!("{id} fechada");
    }
}

fn prefs_window() -> Window {
    Window::new("Preferências").size(460, 320).min_size(360, 240).glass()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let app = Editor { largura_lateral: 220.0, altura_terminal: 160.0, terminal: true, texto: String::new(), fonte: 1, numeros: true, abriu_prefs: false, aviso: String::new(), contado: 0 };
    run(Window::new("Editor").app_id("janelas-filhas").size(900, 560).min_size(560, 360).full_size_content(), app)
}
