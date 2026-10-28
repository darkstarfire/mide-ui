//! Janela comum (sem conteúdo inteiro) com menus: no mide, a barra de título é do
//! compositor e os menus vão para a barra superior; fora do mide (ou com
//! `MIDE_UI_STANDALONE=1`), o mide-ui desenha a barra, com os menus dentro dela.
//!
//! Mostra também a tradução: os textos estão em inglês no código, passam por `tr`/`tr!`
//! e o catálogo pt-BR do app (`PT_BR`, aqui embutido) é registrado com `add_catalog`.

use mide_ui::kit::{i18n::add_catalog, run, tr, window_menu, App, MenuGroup, MenuItem, Ui, Window};

/// Catálogo pt-BR do exemplo (num app de verdade, `include_str!("../locales/pt_BR.po")`).
const PT_BR: &str = r#"
msgid "Regular window"
msgstr "Janela comum"

msgid "{n} click(s). Last menu: {menu}"
msgstr "{n} clique(s). Último menu: {menu}"

msgid "none"
msgstr "nenhum"

msgid "Click"
msgstr "Clicar"

msgid "File"
msgstr "Arquivo"

msgid "New"
msgstr "Novo"

msgid "Quit"
msgstr "Sair"

msgid "Edit"
msgstr "Editar"

msgid "Copy"
msgstr "Copiar"

msgid "Paste"
msgstr "Colar"
"#;

#[derive(Default)]
struct Comum {
    cliques: u32,
    ultimo_menu: String,
}

impl App for Comum {
    fn view(&mut self, ui: &mut Ui) {
        for action in ui.input().menu.clone() {
            self.ultimo_menu = action;
        }
        ui.page(|ui| {
            ui.title(&tr("Regular window"));
            ui.card(|ui| {
                let menu = if self.ultimo_menu.is_empty() { tr("none").into_owned() } else { self.ultimo_menu.clone() };
                ui.label(&mide_ui::tr!("{n} click(s). Last menu: {menu}", n = self.cliques, menu = menu));
                if ui.primary_button(&tr("Click")).clicked() {
                    self.cliques += 1;
                }
            });
        });
    }

    fn menu_bar(&mut self) -> Vec<MenuGroup> {
        vec![
            MenuGroup { title: tr("File").into(), items: vec![MenuItem::action("novo", &tr("New")).shortcut("Ctrl+N"), MenuItem::Separator, MenuItem::action("sair", &tr("Quit"))] },
            MenuGroup { title: tr("Edit").into(), items: vec![MenuItem::action("copiar", &tr("Copy")), MenuItem::action("colar", &tr("Paste"))] },
            window_menu(Vec::new()),
        ]
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    add_catalog("pt_BR", PT_BR);
    run(Window::new(&tr("Regular window")).app_id("janela-comum").size(560, 360), Comum::default())
}
