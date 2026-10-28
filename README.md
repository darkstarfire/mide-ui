# mide-ui

Framework de apps nativos para Linux, em Rust, sobre Wayland, nascido do
[mide](https://github.com/fabioaacarneiro/mide) (Mac Inspired Desktop Environment): janelas e componentes no visual
do macOS Tahoe (barra lateral de vidro, barra de ferramentas em cápsulas, cartões,
interruptores, menus), tema claro/escuro e cor de destaque da sessão, menus na barra
superior do mide, área de transferência, arrastar e soltar, seletor de arquivos, vidro
e várias telas.

**Objetivo:** apps com o visual do Tahoe em qualquer desktop Linux com Wayland (mide,
GNOME, KDE, Hyprland...). Dentro do mide, os apps ganham o que só ele oferece (menus na
barra superior, vidro desfocado pelo compositor, cor de destaque da sessão); fora dele,
o framework desenha tudo sozinho. Hoje o framework ainda depende de alguns recursos do
mide; a independência está em andamento (veja `docs/roadmap.md`).

Criado do zero: o macOS é só inspiração, sem código, imagens, ícones ou fontes da
Apple.

## Começar

O jeito mais rápido é o `mide-new`, que vem com o mide:

```sh
mide-new "Minhas Notas"
cd minhas-notas
cargo run
```

Ou à mão, no `Cargo.toml`:

```toml
[dependencies]
mide-ui = { git = "https://github.com/fabioaacarneiro/mide-ui" }
```

```rust
use mide_ui::kit::{run, App, Ui, Window};

#[derive(Default)]
struct Contador { cliques: u32 }

impl App for Contador {
    fn view(&mut self, ui: &mut Ui) {
        ui.page(|ui| {
            ui.title("Contador");
            ui.label(&format!("Você clicou {} vezes.", self.cliques));
            if ui.primary_button("Clicar").clicked() {
                self.cliques += 1;
            }
        });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run(Window::new("Contador").app_id("contador").size(360, 240).full_size_content(), Contador::default())
}
```

O app roda numa sessão do mide (ou no mide em janela, para desenvolver).

## Documentação

- Guia e lista de componentes: [`docs/guide.md`](docs/guide.md).
- Mapa do visual (raios, espaços, cores, vidro, componentes): [`docs/tahoe.md`](docs/tahoe.md).
- Galeria de componentes: `cargo run --example mide-ui-demo`.

## Organização

- `kit`: componentes, janela, eventos e tema (o que os apps usam).
- `ui`: desenho, texto e ícones de baixo nível.
- `platform`, `shell`, `applications`, `storage`: auxiliares do desktop que o kit usa
  e que também servem aos apps (configuração do mide, menus na barra, ícones do tema,
  apps instalados, pastas do usuário, miniaturas).
- `platform::media` e `kit::media_controls`: leitura MPRIS, capas e um painel reutilizável
  para controlar players compatíveis, com fila e playlists quando disponíveis.

## Licença

À sua escolha, [MIT](LICENSE-MIT) ou [Apache-2.0](LICENSE-APACHE): pode ser usado em
apps abertos ou fechados.
