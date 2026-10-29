# mide-ui: crie apps com a interface do mide

> **Antes de criar uma tela, leia o guia de identidade visual: [`design.md`](design.md).** Ele define cores, vidro, estados, menus, diálogos e as animações padrão (durações e curvas) que todo app do mide segue.

O mide-ui é o framework para criar apps com a mesma cara e o mesmo comportamento das
Configurações do Sistema do mide: cartões brancos, interruptores, trilhos, barra
lateral, barra de rolagem fina e a barra de título com os três botões. Você escreve
em Rust, descreve a tela numa função e o framework cuida da janela, do mouse, do
teclado e do desenho.

## O pacote e o `mide-new`

O mide-ui é um projeto próprio (github.com/fabioaacarneiro/mide-ui): um app depende
só dele, sem o compositor do mide.

```toml
[dependencies]
mide-ui = { git = "https://github.com/fabioaacarneiro/mide-ui" }
```

O jeito mais rápido de começar é o `mide-new`, que cria um projeto pronto: `Cargo.toml`,
`src/main.rs` com uma janela do Tahoe (barra lateral de vidro com seções, barra de
ferramentas, cartões, campo de texto, lista, interruptor e menus na barra superior), o
`.desktop` e um README com como instalar.

```sh
mide-new "Minhas Notas"            # cria ./minhas-notas
mide-new "Minhas Notas" --dir ~/Projetos
mide-new "Minhas Notas" --local    # usa o mide-ui da pasta ao lado (desenvolvimento)
```

Dentro do mide, os apps usam o mesmo framework por `mide::kit` (repassado pelo
pacote `mide`); fora dele, por `mide_ui::kit`.

## Primeiro app

```rust
use mide_ui::kit::{run, App, Ui, Window};

#[derive(Default)]
struct Contador {
    cliques: u32,
}

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
    run(Window::new("Contador").size(360, 240), Contador::default())
}
```

Rode dentro de uma sessão do mide (o app é uma janela Wayland comum).

## Como funciona

O `view` é chamado a cada quadro e **descreve a tela inteira**. Não há eventos para
ligar nem objetos de tela para guardar: o estado é o seu `struct`, e os componentes
dizem o que aconteceu com eles naquele quadro.

```rust
if ui.button("Salvar").clicked() {
    self.salvar();
}
ui.switch("Notificações", &mut self.notificacoes);   // troca o bool sozinho
```

O quadro é redesenhado quando há mouse, teclado ou uma mudança. Quando um clique
muda o estado, o framework desenha de novo na hora, para a tela já mostrar o valor
novo.

## Componentes

### Controles de mídia

`platform::media` fornece uma sessão independente do player (Spotify, navegadores e
players locais que exportem MPRIS), metadados, capa, estado, fila e playlists quando o
player oferece essas extensões. Use `active_with_details(false)` para monitorar sem
carregar listas; consulte com `true` enquanto o painel estiver aberto. `load_artwork`
carrega capas `file://` ou HTTP(S) em uma imagem pequena.

`ui.media_controls(&sessao, capa)` desenha o cartão reaproveitável e devolve as ações
solicitadas. O app executa cada uma com `platform::media::command(&sessao, acao)`;
`platform::media::seek` atualiza a posição. As listas de reprodução e a fila são
opcionais no MPRIS e aparecem somente quando o player as fornece.

```rust,ignore
let sessao = mide_ui::media::active_with_details(true).unwrap();
let acoes = ui.media_controls(&sessao, capa.as_ref());
for acao in acoes {
    mide_ui::media::command(&sessao, acao)?;
}
```

| Componente | O que faz | Resposta útil |
|---|---|---|
| `ui.title("Texto")` | Título grande em negrito | — |
| `ui.heading("Grupo")` | Nome pequeno e cinza acima de um cartão | — |
| `ui.label("Texto")` | Texto comum, quebra em linhas | — |
| `ui.note("Texto")` | Texto menor e cinza (explicações) | — |
| `ui.button("Texto")` | Botão comum | `.clicked()` |
| `ui.primary_button("Texto")` | Botão de destaque (ação principal) | `.clicked()` |
| `ui.switch("Texto", &mut bool)` | Linha com interruptor | `.changed()` |
| `ui.slider("Texto", &mut f32, min, max)` | Linha com trilho | `.changed()` |
| `ui.text_input(&mut String, "dica")` | Campo de texto de uma linha | `.changed()`, `.submitted()` (Enter) |
| `ui.list_item("Texto", "detalhe")` | Item de lista clicável | `.clicked()` |
| `ui.choice_item("Texto", "detalhe", marcado, ativo)` | Item de uma lista de escolha: o marcado numa pílula azul, o desativado em cinza e sem clique | `.clicked()` |
| `ui.icon("nome-do-icone", 24)` | Ícone do tema do usuário | — |
| `ui.separator()` | Linha fina | — |
| `ui.space(12.0)` | Espaço vazio | — |

| `ui.tile(Tile::Icon("folder"), "Nome", selecionado, 64)` | Ícone (ou `Tile::Image(&pixmap)`) com o nome embaixo em até duas linhas (cortado no meio, guardando a extensão; escolhido, aparece inteiro; o ponteiro em cima mostra o nome completo), como no Finder | `.clicked()`, `.double_clicked()` |
| `ui.list_header_resizable("Nome", &mut colunas, ordem, 60.0)` | Cabeçalho de lista com as divisórias arrastáveis (o Nome cresce quando as outras colunas encolhem); nomes cortados em `tree_row`/`list_row` mostram o inteiro numa dica | coluna clicada; `colunas` com as larguras novas |
| `ui.list_row("folder", "Nome", &[("ontem", 170.0)], selecionada)` | Linha de lista com ícone e colunas à direita | `.clicked()`, `.double_clicked()` |
| `ui.icon_button("go-previous", "‹", ativo)` | Botão pequeno com ícone do tema (texto de reserva) | `.clicked()` |
| `ui.icon_toggle("view-list-symbolic", "☰", marcado)` | Botão de ícone que fica marcado | `.clicked()` |
| `ui.tool_button("document-open-symbolic", "Abrir arquivo")` | Botão só de ícone com a **dica obrigatória** (I2): aparece com o ponteiro parado sobre ele; sem o ícone, a primeira letra da dica | `.clicked()` |
| `ui.tool_toggle("utilities-terminal-symbolic", "Terminal", aberto)` | O mesmo, marcado quando `aberto` | `.clicked()` |
| `ui.tool_group(&[Tool::new("document-open-symbolic", "Abrir arquivo"), Tool::new("folder-open-symbolic", "Abrir pasta")])` | Grupo de botões numa cápsula da barra de ferramentas do Tahoe, cada um com a dica (`Tool::on`, `Tool::enabled`) | índice clicado |
| `ui.strong("Texto")` | Texto em negrito numa linha só (corta com "…") | — |

| `ui.scroll_area(240.0, \|ui\| ...)` | Área de altura fixa que rola (listas dentro de diálogos) | — |
| `ui.scroll_area_showing(240.0, Some((topo, base)), \|ui\| ...)` | A mesma, rolando o mínimo para a faixa (do topo do conteúdo) ficar à vista: o item marcado pelo teclado | — |
| `ui.reveal(resposta.rect)` | Pede que a área que rola em volta (página ou `scroll_area`) role o mínimo para o item ficar à vista (chamar uma vez: o arquivo pedido ao abrir o Arquivos) | — |
| `ui.dropdown("Texto", &opções, &mut índice)` | Linha com o valor escolhido; o clique abre a lista | `.changed()` |

### Controles do Tahoe (`docs/tahoe.md`)

| Componente | O que faz | Resposta útil |
|---|---|---|
| `ui.button_styled("Texto", ButtonStyle::Prominent)` | Botão em cápsula: `Prominent` (azul), `Bordered` (comum), `Plain` (só texto), `Glass` (vidro), `Destructive` (vermelho). `button` e `primary_button` também são cápsulas | `.clicked()` |
| `ui.segmented(&["A", "B"], &mut índice, Some(largura))` | Controle segmentado: a pílula desliza até a opção escolhida | `.changed()` |
| `ui.checkbox("Texto", &mut bool)` | Caixa de seleção com ✓ | `.changed()` |
| `ui.radio("Texto", marcado)` | Botão de opção; quem chama troca a escolha no clique | `.clicked()` |
| `ui.stepper(&mut i64, min, max)` | Número com ▲▼ em cápsula | `.changed()` |
| `ui.progress_bar(Some(0.6))` / `ui.progress_bar(None)` | Barra de progresso (com `None`, uma faixa que anda) | — |
| `ui.badge("3")` | Selo vermelho com número ou texto curto | — |
| `ui.tag_dots(&[TAG_COLORS[0]])` | Bolinhas de etiqueta (cores do Finder em `TAG_COLORS`) | — |
| `ui.toolbar_buttons(&[(ícone, reserva, marcado, ativo)])` | Grupo de botões numa cápsula de vidro (barra de ferramentas) | índice clicado |
| `ui.toolbar_title("Título", largura_máx)` | Título da janela na barra de ferramentas (encolhe com "…") | — |
| `ui.search_field(&mut String, "Buscar")` | Busca em cápsula, com lupa e ✕ | `.changed()` |
| `ui.window_controls(x, y)` | Os três botões da janela (só em janela de conteúdo inteiro; a barra lateral e a barra de ferramentas já põem). O verde entra em tela cheia (com ⌥ maximiza, como no Mac); em tela cheia os botões somem (`ui.fullscreen()`), e no mide o compositor mostra a barra de título dele ao levar o ponteiro ao topo | — |
| `ui.window_drag_area(rect)` | O vazio do retângulo arrasta a janela; duplo clique maximiza (chamar depois dos controles da área) | — |
| `ui.remaining_width()` | Largura que sobra na linha atual (para encolher um título) | — |
| `ui.tab_bar(&títulos, escolhida)` | Barra de abas em cápsula, × ao passar o ponteiro e + no fim | `TabAction::Select/Close/New` |
| `ui.tooltip(&resposta, "texto")` | Dica que aparece depois de o ponteiro parar 0,8 s sobre o componente | — |
| `ui.popover(&mut aberto, ui.to_window(resposta.rect), largura, \|ui\| ...)` | Painel de vidro com ponta, preso a um botão; fecha com clique fora ou Esc (chamar no fim do `view`) | — |
| `ui.to_window(rect)` | Retângulo de uma área (que rola) nas coordenadas da janela | — |

A seleção das listas, da árvore e da grade é uma pílula de cantos 8 na cor de destaque,
e fica cinza quando a janela perde o foco (como no Mac).

Todas as respostas também trazem `.hovered()`, `.double_clicked()`,
`.secondary_clicked()` (botão direito) e o lugar ocupado (`rect`).

## Menu de contexto

Qualquer componente ganha um menu com o botão direito. O menu fica por cima de tudo,
fecha ao escolher, ao clicar fora ou com Esc:

```rust
let r = ui.tile(Tile::Icon("text-x-generic"), "notas.txt", false, 64);
ui.context_menu(&r, |m| {
    if m.item("Abrir") { self.abrir(); }
    if m.item("Renomear…") { self.renomear(); }
    m.separator();
    if m.danger("Mover para a Lixeira") { self.apagar(); }   // texto em vermelho
});
```

Para o menu do "vazio" (fundo de uma pasta), use `ui.background(ui.region())` depois
dos itens: ele só reage onde não há componente.

## Teclado, foco e título

- `ui.input().key(Key::Delete)`, `ui.input().shortcut("n")` (Ctrl+N).
- `ui.focus_next_input()`: o próximo `text_input` já começa com o foco e o texto todo
  selecionado (digitar substitui, como ao renomear no Finder). `ui.typing()` diz se
  há um campo com foco (para não tratar as teclas duas vezes).
- `ui.set_title("Downloads")`: troca o título da janela.
- `ui.request_tick()`: pede outro quadro em 40 ms, para mostrar trabalho feito em
  segundo plano (por exemplo, miniaturas que chegam de outra thread).

## Layout

Os componentes se empilham de cima para baixo, ocupando a largura disponível.

- **`ui.row(|ui| ...)`**: põe os itens lado a lado (botões numa mesma linha).
- **`ui.card(|ui| ...)`**: agrupa num cartão branco; os itens dentro ganham linhas finas
  entre si, como nas Configurações.
- **`ui.page(|ui| ...)`**: área com margem que rola quando o conteúdo não cabe, com a
  barra de rolagem como a do macOS: por cima do conteúdo (não ocupa lugar no layout),
  fina, aparece ao rolar ou com o ponteiro perto da borda, engrossa com o ponteiro em
  cima (e se arrasta ou se clica no trilho) e some esmaecendo depois de 1 s. Vale para
  todas as áreas que rolam do kit (`scroll_area`, barra lateral, área de texto,
  seletores). `[appearance] scrollbars = "always"` deixa a barra sempre à mostra.
- **`ui.wrap(|ui| ...)`**: lado a lado, quebrando linha quando a largura acaba (grade
  de ícones).
- **`ui.toolbar(|ui| ...)`**: faixa de ferramentas no topo, com linha fina embaixo.
  Dentro dela, `ui.push_right(largura)` manda o resto para a direita e
  `ui.sized(largura, |ui| ...)` fixa a largura do próximo item (um campo de busca).
- **`ui.sidebar(&secoes, atual, |ui, secao| ...)`**: barra lateral com seções (nome e
  ícone do tema) e o conteúdo da seção à direita, numa página que rola. Devolve a seção
  escolhida:

```rust
let secoes = [("Geral", "preferences-system"), ("Sobre", "help-about")];
self.secao = ui.sidebar(&secoes, self.secao, |ui, secao| match secao {
    0 => self.geral(ui),
    _ => self.sobre(ui),
});
```

## Janela

```rust
Window::new("Meu App")      // título; o id vira "meu-app"
    .app_id("org.exemplo.MeuApp")   // id para a dock e o launcher
    .size(760, 520)
    .min_size(480, 320)
    .theme(Theme::default())
```

**Janela do Tahoe:** `.full_size_content()` tira a barra de título do compositor e
deixa os cantos com 24. Com `ui.glass_sidebar()` + `ui.sidebar_area(...)` a barra
lateral vira um painel de vidro flutuante com os botões da janela dentro; sem barra
lateral, `ui.toolbar(...)` põe os botões no começo. O vazio das duas arrasta a janela.
O exemplo `mide-ui-demo` é a galeria de tudo isso.

Para o app aparecer no launcher e poder ser fixado na dock, instale um arquivo
`.desktop` com o mesmo id em `StartupWMClass`.

**Janela de vidro:** `.glass()` troca o fundo opaco pelo vidro de painel do mide (o
compositor desfoca o que está atrás; o véu é o mesmo da barra lateral do Arquivos).
Cartões e controles continuam opacos por cima. Com o vidro desligado nas
Configurações (ou fora do mide), o fundo volta a ser opaco na hora.

### Janelas filhas (Preferências, inspetores)

Um app pode abrir janelas próprias que pertencem à principal: o compositor as mantém
por cima dela (`xdg_toplevel.set_parent`) e elas fecham junto com o app. O app pede a
janela com um id e desenha o conteúdo dela em `App::window_view`:

```rust
impl App for Editor {
    fn view(&mut self, ui: &mut Ui) {
        if ui.tool_button("preferences-system-symbolic", "Preferências").clicked() {
            // Pedir de novo uma janela já aberta não faz nada.
            ui.open_window("prefs", Window::new("Preferências").size(520, 440).glass());
        }
    }

    fn window_view(&mut self, id: &str, ui: &mut Ui) {
        if id == "prefs" {
            ui.page(|ui| {
                ui.title("Preferências");
                ui.switch("Números de linha", &mut self.numeros);
                if ui.primary_button("Fechar").clicked() {
                    ui.close(); // dentro da filha, fecha só ela
                }
            });
        }
    }

    // Opcionais: perguntar antes de fechar e saber quando fechou.
    fn window_close_requested(&mut self, _id: &str) -> bool { true }
    fn window_closed(&mut self, _id: &str) { self.gravar_preferencias(); }
}
```

- `ui.open_window(id, Window)`, `ui.close_window(id)`, `ui.is_window_open(id)` e
  `ui.window_id()` (`None` na principal, `Some(id)` numa filha).
- A filha usa o `app_id` da principal (a dock mostra um ícone só); da `Window` valem
  título, tamanhos, `full_size_content`, `fixed_size`, `glass` e `accepts_files`.
- Cada janela tem a própria entrada (ponteiro, teclado, área de transferência,
  arquivos soltos) e a própria memória dos componentes; o estado do app é um só (o
  mesmo `self` nos dois `view`).
- Vidro numa filha: as regiões vão ao compositor com o nome `app:<app_id>><título>`
  (espaços como `%20`); a principal continua `app:<app_id>`. Só vale com um
  compositor que responde `child_glass` ao comando `features` do canal de controle
  (`mide_ui::control_client::compositor_has`); sem isso, a filha fica opaca (o vidro
  dela iria para a principal, que tem o mesmo `app_id`).

### Barra lateral, divisórias e ponteiro

```rust
// Barra lateral de vidro com a largura do app (arrastável de 160 a 420):
let r = ui.sidebar_panel(&mut self.largura, 160.0, 420.0, |ui| self.arvore(ui), |ui| {
    // À direita: editor em cima e terminal embaixo, com outra divisória.
    ui.split_vertical(&mut self.altura_terminal, 80.0, 400.0, |ui| self.editor(ui), |ui| self.terminal(ui));
});
if r.submitted() {
    self.config.largura = self.largura; // o arrasto terminou: hora de gravar
}
```

- `ui.split(&mut largura, min, max, esquerda, direita)` e
  `ui.split_vertical(&mut altura_de_baixo, min, max, cima, baixo)`: dois painéis até o
  fim da área, cada um recortado no seu retângulo (as coordenadas começam no canto
  dele), com a divisória arrastável. A resposta tem `changed` enquanto arrasta e
  `submitted` quando o arrasto termina.
- `ui.sidebar_panel(...)`: o mesmo `split` com a barra lateral de vidro do mide; numa
  janela de conteúdo inteiro, flutuante, com os botões da janela no topo (o conteúdo
  dela começa abaixo deles) e o vazio arrastando a janela.
- `ui.glass_fill(retângulo, raio)`: o vidro de painel em qualquer retângulo (devolve
  `false` e pinta opaco com o vidro desligado).
- `ui.set_cursor(Cursor::ResizeHorizontal)`: forma do ponteiro enquanto ele está na
  janela (vale no quadro em que é pedida; as divisórias e o cabeçalho de colunas já
  pedem). Usa o `wp_cursor_shape` do compositor ou, sem ele, o tema de cursores.

### Ícones

Os apps do mide usam sempre os ícones do mide. A busca (`ui.icon`, `icon_button`,
`tool_button`...) segue esta ordem:

1. os **símbolos embutidos** no mide-ui (`mide_ui::icons::SYMBOLS`): desenho nosso,
   o mesmo em qualquer desktop. Hoje: `document-open-symbolic` (abrir arquivo: um
   documento), `document-new-symbolic`, `document-save-symbolic`,
   `folder-open-symbolic` (abrir pasta: uma pasta), `edit-find-symbolic`,
   `preferences-system-symbolic`, `view-sidebar-start-symbolic`, `view-dual-symbolic`
   (painel de baixo), `utilities-terminal-symbolic`, `list-add-symbolic` e
   `window-close-symbolic`;
2. o tema do mide (`mide-icons`), quando instalado;
3. o tema do sistema (gsettings) e o Adwaita, só como reserva;
4. o `hicolor`.

### Textos e tradução

As dicas dos botões passam por `mide_ui::kit::tr`, o ponto de entrada da tradução
(I1). Hoje ele devolve o próprio texto; quando os catálogos de idioma chegarem, todo
texto que já passa por `tr` passa a ser traduzido sem mudar os apps. Textos novos da
interface devem passar por `tr`.

O exemplo `janelas-filhas` junta tudo isso (`cargo run --example janelas-filhas`).

## Camadas e painéis de vidro

Um app pode ser uma **camada** por cima de tudo (como o buscador do mide), em vez de
janela: transparente, na tela inteira e com o teclado só para ela.

```rust
run(Window::new("Buscar").overlay("meu-painel"), MeuPainel::default())
```

Dentro dela, `ui.glass_panel` desenha um painel com o vidro do mide (o compositor
desfoca o que está atrás e aplica o vidro líquido) e a altura acompanha o conteúdo:

```rust
let (_, _, w, h) = ui.region();
ui.glass_panel((w - 600.0) / 2.0, h * 0.2, 600.0, 18.0, |ui| {
    ui.text_input(&mut self.busca, "Buscar");
    for (i, r) in self.resultados.iter().enumerate() {
        if ui.result_row(&r.icone, &r.titulo, &r.tipo, i == self.escolhido).clicked() {
            ui.close();   // fecha o app
        }
    }
});
```

Ícones são carregados em segundo plano: o componente aparece sem o ícone por um
instante e o ícone surge assim que fica pronto. Para já ter os ícones na primeira
vez, carregue numa thread e entregue com `ui.provide_icon(nome, tamanho, imagem)`.

### Camada no canto (avisos que somem sozinhos)

Com `.corner("nome")`, o app é uma camada pequena, do tamanho de `.size(l, a)`, no canto
de baixo à direita da tela principal, acima da dock, e **sem o teclado** (não tira o
foco de quem está digitando). É a miniatura que aparece depois de uma captura de tela.
O app diz onde recebe cliques com `ui.input_area(retângulo)` a cada quadro; o resto da
camada deixa os cliques passarem.

```rust
run(Window::new("Aviso").corner("meu-aviso").size(280, 300), MeuAviso::default())
```

Um menu de contexto numa camada pequena precisa de duas ajudas: `ui.menu_open()` diz se
há um menu aberto (e o retângulo dele), para a área de cliques crescer enquanto ele
aparece, e `ui.close_menu()` fecha o menu (o clique fora da camada não chega ao app;
feche quando o ponteiro sai dela).

### Camada de fundo (área de trabalho)

Com `.desktop("nome")`, o app é uma camada atrás das janelas e na frente do papel de
parede (os ícones da área de trabalho), que só pega o teclado quando clicada. Um
diálogo desenhado nela ficaria escondido atrás das janelas: enquanto ele aparece, chame
`ui.set_raised(true)` (a camada vem para a frente e pega o teclado) e, quando fechar,
`ui.set_raised(false)`. Pode chamar a cada quadro: só muda quando o valor muda.

```rust
ui.set_raised(self.dialogo.is_some());
```

### Uma camada em cada tela

Com `.on_every_output()` (junto com `.overlay`), a camada cobre todas as telas, e não
só a principal (a captura de tela, que escurece todas):

```rust
run(Window::new("Telas").overlay("minhas-telas").on_every_output().fixed_scale(), App::default())
```

O mesmo `view` é chamado uma vez por tela em cada quadro:

- `ui.screen()` é a tela deste passe (`Screen`: nome, origem no desktop, tamanho em
  pontos, escala e se é a principal); `ui.screens()` lista todas, primeiro a da
  superfície principal do app (a tela principal de quando o app abriu: trocar a
  principal com o app aberto não muda a camada de lugar).
- `ui.mirror()` é `true` nos passes das outras telas. **Estado só no passe com
  `!ui.mirror()`**: teclas, cliques e mudanças do app ficam lá; os outros passes só
  desenham (o que eles pedem — fechar, vidro, título — é descartado).
- `ui.pointer_global()` dá o ponteiro em coordenadas do desktop, em qualquer tela; um
  arrasto que começa numa tela e termina na outra continua chegando ao app.
- Só a camada da principal recebe o teclado. As outras desenham em escala 1 (o
  compositor amplia nas telas com escala) e se chamam `<namespace>@<TELA>`.
- Monitores que entram e saem ganham e perdem a camada com o app aberto. Se sai (ou é desligada)
  a tela da superfície principal, o app fecha: a camada dela ficaria invisível com o
  teclado.

As funções de geometria (`screen_at`, `union`, `to_main`...) ficam em
`mide::kit::screens`. O exemplo `examples/kit-telas.rs` mostra tudo isso.

## Área de texto (várias linhas)

`ui.text_area(&mut doc, altura)` é o editor do Editor de Texto: números de linha, fonte
de largura fixa, cursor, seleção com mouse e teclado, desfazer (Ctrl+Z) e refazer
(Ctrl+Shift+Z, Ctrl+Y), área de transferência (Ctrl+A/C/X/V, também com Shift) e
rolagem; só as linhas à vista são desenhadas. Os outros atalhos com Ctrl (salvar,
buscar...) ficam para o app. O documento (`TextDoc`) guarda o texto, o cursor e o
histórico; `version` aumenta a cada mudança (começa em 0 em cada `TextDoc::new`) e
`can_undo()`/`can_redo()` servem ao menu Editar. Quando o app muda o texto por conta
própria, use `type_text` ou chame `before_change` antes: senão a mudança não entra no
desfazer e o `version` não muda.

`ui.text_area_with(&mut doc, altura, TextAreaLook { .. })` muda a aparência: sem os
números de linha, em fonte proporcional, outro tamanho, uma dica com o documento vazio
(cortada com "…" se não couber) e decorações, pedidas a cada quadro depois das teclas
(nunca atrasam um quadro). As decorações ficam fora do texto (cursor, seleção e clique
não mudam):

- `colors`: trechos coloridos;
- `underlines`: trechos sublinhados com pontilhado vermelho (erros);
- `trailing`: um texto depois do fim de uma linha (o resultado das notas da
  Calculadora), 4 pt depois do fim; vários na mesma linha vêm na ordem da lista. Com o
  cursor no fim dessa linha, a área rola para ele ficar à vista; o que não cabe até a
  borda termina com "…".

Índices em bytes do texto; trechos fora do texto ou no meio de um caractere são
ignorados. A resposta traz, no quadro de um clique (esquerdo ou direito), a posição do
texto sob o ponteiro: guarde a linha no clique direito para o menu de contexto usar
depois (com o menu aberto, o ponteiro está em cima dele e a posição não vem):

```rust
use mide_ui::kit::{Decorations, TextAreaLook, TextDoc};
use mide_ui::kit::textarea::{line_of, line_starts};

let destaque = ui.theme().accent;
let mut decorar = |doc: &TextDoc| Decorations {
    colors: vec![(0..5, destaque)],                  // "total" em azul
    underlines: vec![],
    trailing: vec![(0, "1.200".into(), destaque)],   // depois do fim da linha 0
};
let r = ui.text_area_with(&mut self.doc, 300.0, TextAreaLook {
    line_numbers: false,
    sans: true,
    size: Some(15.0),
    placeholder: "Digite uma conta e termine com =",
    decorate: Some(&mut decorar),
});
if r.response.secondary_clicked {
    self.linha_do_menu = r.pointer_index.map(|i| line_of(&line_starts(&self.doc.text), i));
}
ui.context_menu(&r.response, |m| {
    if m.item("Copiar o resultado") { /* usa self.linha_do_menu */ }
});
```

## Diálogos e seletor de arquivos

`ui.modal(largura, |ui| ...)` escurece a janela e mostra um painel no meio; só o que
está dentro dele reage. Para o caso comum há o aviso pronto, que devolve o botão
clicado (o último é o principal; Esc conta como o primeiro e Enter como o último):

```rust
if self.confirmar {
    match ui.alert("Apagar o arquivo?", "Não dá para desfazer.", &["Cancelar", "Apagar"]) {
        Some(1) => { self.apagar(); self.confirmar = false; }
        Some(_) => self.confirmar = false,
        None => {}
    }
}
```

Numa janela estreita (a Calculadora básica, 250 pt), os botões do aviso encolhem para
caber; estreitos demais, ficam um embaixo do outro, com o principal em cima.

Para montar um diálogo próprio: `ui.dialog_buttons(&["Cancelar", "OK"])` põe os botões à
direita, cada um do tamanho do rótulo, com o último em azul (devolve o clicado);
`ui.error_note("Senha incorreta")` escreve um erro pequeno em vermelho;
`ui.password_field(&mut senha, "Senha", erro)` é o campo de senha com a cara dos campos
do tema (bolinhas, sempre com o teclado, sem copiar; Enter responde `submitted`, Esc e
Ctrl+U apagam; `erro` pinta a borda de vermelho). O `ui.password_input` é o da tela de
bloqueio, claro sobre o papel de parede. O `mide-askpass` usa os três.

Para escolher numa lista longa, em grupos e com busca (as 140 unidades da Calculadora,
em 15 categorias), há o seletor de lista (`ListPicker`): sem busca, mostra o grupo do item
atual, com "‹ Todos os grupos" no topo; com busca (sem acento nem maiúscula, todas as
palavras, também pelo nome do grupo), uma lista só. ↑/↓ andam (pulando os itens
desativados), Enter escolhe o marcado (sem busca, o item atual; com busca, o melhor
resultado: a palavra exata, depois o começo do nome), Esc e "Cancelar" fecham:

```rust
use mide_ui::kit::{ListPicker, PickGroup, PickItem, Picked};

// Rótulo, detalhe cinza e as palavras exatas do item (símbolo, apelidos).
let grupos = vec![PickGroup { name: "Comprimento".into(), items: vec![PickItem::new("Metros", "m", &["m", "metro"])] }];
self.escolher = Some(ListPicker::new("Converter de", "Todas as categorias", "Buscar unidade", grupos, (0, 0)));
// a cada quadro, por cima do resto:
if let Some(seletor) = self.escolher.as_mut() {
    match seletor.show(ui) {
        Some(Picked::Chosen { group, item }) => { self.usar(group, item); self.escolher = None; }
        Some(Picked::Cancelled) => self.escolher = None,
        None => {}
    }
}
```

O seletor de arquivos (`FileChooser`) abre por cima da janela: lugares (pasta pessoal,
Área de trabalho, Documentos, Downloads, Imagens), a pasta de cima, a lista com pastas
primeiro, duplo clique para entrar ou escolher. `FileChooser::open` escolhe um arquivo,
`FileChooser::save` pede o nome (e pergunta antes de substituir um que já existe) e
`FileChooser::folder` escolhe uma pasta; `.extensions(&["txt", "md"])` filtra. O app
guarda o seletor enquanto ele está aberto:

```rust
if let Some(seletor) = self.abrir.as_mut() {
    match seletor.show(ui) {
        Some(Choice::Chosen(caminho)) => { self.abrir_arquivo(caminho); self.abrir = None; }
        Some(Choice::Cancelled) => self.abrir = None,
        None => {}
    }
}
```

Mais opções (as do portal `FileChooser`, usadas pelo `mide-chooser`, que é o seletor dos
outros apps na sessão do mide): `.multiple(true)` deixa escolher vários com Ctrl+clique
e Shift+clique (todos em `seletor.chosen()`, depois do `Chosen` com o primeiro);
`.filters(vec![Filter { nome, padrões }], atual)` mostra a lista "Mostrar" com filtros
por nome (`Pattern::Glob("*.png")`, sem diferenciar maiúsculas) ou por tipo
(`Pattern::Mime("image/*")`), e `seletor.current_filter()` diz o escolhido;
`.choices(vec![ExtraChoice { .. }])` põe caixas de marcar ou listas embaixo
(`seletor.choice_values()`); `.accept_label("Enviar")` troca o texto do botão;
`FileChooser::save_files(título, pasta, &nomes)` escolhe a pasta onde salvar vários
arquivos (pergunta antes de substituir); `.embedded(true)` ocupa a janela inteira em vez
de abrir por cima.

## Tema

O tema segue Configurações → Aparência (claro ou escuro) e troca na hora, sem
reabrir o app. `Theme::system()` é o tema escolhido; `Theme::dark()` e
`Theme::default()` (claro) existem para quem quer um fixo. `theme.shade(alfa)` dá o
tom de contraste certo para o tema (preto no claro, branco no escuro): use-o em
linhas, trilhos e fundos sutis em vez de cores fixas.

Cores, cantos, fontes e espaçamentos ficam em `Theme`. Para mudar só a cor de
destaque:

```rust
let mut tema = Theme::default();
tema.accent = [52, 199, 89, 255];   // verde
run(Window::new("Verde").theme(tema), MeuApp::default())
```

Na barra lateral, um item sem ícone vira um título de grupo (`("Favoritos", "")`).
`ui.sidebar_area` é igual, mas entrega a área inteira à direita, sem margem nem
rolagem: dá para pôr uma `toolbar` fixa e uma `page` embaixo.

## Exemplo completo

`examples/mide-ui-demo.rs` usa os componentes básicos: barra lateral, cartões com
interruptores e trilho, campo de texto com Enter e uma lista de tarefas.

O app **Arquivos** (`src/bin/mide-files.rs`) é um app completo feito só com o mide-ui:
barra lateral com grupos, barra de ferramentas, grade de ícones com miniaturas,
lista, menus de contexto, duplo clique, renomear e atalhos.

```
cargo run --example mide-ui-demo
```

## Ainda não tem (próximas versões)

- Diálogos modais em janela própria (hoje, `ui.modal` dentro da janela; as janelas
  filhas não bloqueiam a principal).
- Seleção múltipla e arrastar e soltar entre componentes.
- Animações dos componentes (o interruptor deslizando).
- Toque na tela (gestos) e acessibilidade.

## Compatibilidade com conceitos do Mac (plano futuro E10)

O mide-ui segue os conceitos do AppKit do macOS, para que uma camada de compatibilidade futura (rodar apps de .dmg por meio de algo como o Darling, com a parte gráfica desenhada pelo mide-ui) possa casar um com o outro. Nada de código, frameworks ou imagens da Apple: só os conceitos. Equivalências de hoje:

| mide-ui | AppKit |
|---|---|
| `Window` (`full_size_content`, `fixed_size`) | `NSWindow` (style mask: titled, fullSizeContentView, sem resizable) |
| `window_controls`, `window_drag_area` | botões padrão da janela, `isMovableByWindowBackground` |
| `toolbar`, `toolbar_buttons`, `toolbar_title`, `search_field` | `NSToolbar`, `NSToolbarItemGroup`, `NSSearchToolbarItem` |
| `glass_sidebar`, `sidebar_search` | `NSSplitViewController` com a barra lateral |
| `App::menu_bar`, `MenuGroup`, `MenuItem` | `NSMenu`, `NSMenuItem` (ações e atalhos) |
| `context_menu`, `popover`, `tooltip` | menu de contexto, `NSPopover`, tooltip |
| `alert`, `modal` | `NSAlert`, folha (sheet) |
| `FileChooser` | `NSOpenPanel`, `NSSavePanel` |
| `copy`, `request_paste`, `start_drag` | `NSPasteboard`, arrastar e soltar |
| `button_styled` (`ButtonStyle`), `switch`, `segmented`, `checkbox`, `radio`, `stepper`, `slider`, `progress_bar` | `NSButton` (bezel styles), `NSSwitch`, `NSSegmentedControl`, `NSStepper`, `NSSlider`, `NSProgressIndicator` |
| `list_row`, `tree_row`, `tile` | `NSTableView`, `NSOutlineView`, `NSCollectionView` |
| `richtext` (trechos de estilo, RTF, PDF) | `NSAttributedString`, `NSTextView`, RTF, `NSPrintOperation` |
| `columns`, `form_row`, `fixed_label` | `NSStackView`, `NSGridView` (formulário com rótulos alinhados) |

Regra: todo componente novo do kit ganha aqui a linha do equivalente no AppKit.


## Menus: vidro, submenus e largura

Os menus de contexto (`ui.context_menu`) são de vidro, como no Tahoe: o conteúdo da
janela atrás deles aparece desfocado e clareado (numa camada transparente, como a área
de trabalho, o compositor desfoca o que está atrás). A largura cresce até o texto mais
longo caber. `m.submenu(ícone, "Abrir com", |m| { ... })` abre um submenu ao lado, com
a setinha "›", quando o ponteiro passa em cima (um nível).

## Seleção em bloco

Antes de cada linha escolhida de `tree_row`/`list_row`, `ui.join_selection(acima,
abaixo)` diz se as vizinhas também estão escolhidas: linhas seguidas viram um bloco só,
com os cantos arredondados só em cima da primeira e embaixo da última (como no Finder).
O azul da seleção é o tom mais forte da cor de destaque (`selection_tone`).

## Receber arquivos arrastados

`Window::accepts_files()` faz a janela aceitar arquivos arrastados de outros apps: o
compositor mostra o ponteiro de "copiar" sobre ela e os caminhos chegam em
`input().dropped`. Janelas sem isso deixam o arrasto passar (a seta comum).


## Editor de código

`TextAreaLook` tem `current_line` (destacar a linha do cursor) e `indent` (o que a tecla
Tab insere: espaços ou `\t`). `ui.set_mono_font(Some("JetBrains Mono"))` troca a fonte
de largura fixa da janela e `ui.mono_fonts()` lista as instaladas. `ui.take_key(Key::Down)`
tira uma tecla do quadro antes dos componentes seguintes (um popup de completar usa as
setas antes da área de texto).
