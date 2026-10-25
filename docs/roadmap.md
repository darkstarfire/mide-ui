# Plano do mide-ui

## Feito (vindo do mide)

Janela de conteúdo inteiro do Tahoe (luzes, arrastar, maximizar), barra lateral de
vidro, barra de ferramentas em cápsulas, busca em cápsula, cartões, botões, interruptor,
deslizante, segmentado, caixa de seleção, rádio, passo, progresso, selo, etiquetas,
menus de contexto, popover, dica, abas, alertas, seletor de arquivos, campo e área de
texto completos, listas, árvore e grade só com o visível, área de transferência,
arrastar e soltar entre apps, menus na barra superior (dbusmenu), tema claro/escuro e
cor de destaque, escala fracionária, várias telas, camadas (layer-shell).

## Para funcionar em qualquer desktop

Feito (2026-10-25): o framework sabe se está no mide (`control_client::on_mide`;
`MIDE_UI_STANDALONE=1` força o modo de fora para testar). Fora do mide:

- **barra de título própria** no visual do Tahoe nas janelas comuns (luzes, menus do
  app dentro dela, título, arrastar e duplo clique para maximizar) — exemplo
  `examples/janela-comum.rs`;
- **cantos arredondados pelo app** (12 nas janelas comuns, 24 nas de conteúdo inteiro);
- **tema e cor de destaque do sistema** pelo portal do freedesktop
  (`org.freedesktop.appearance`: `color-scheme` e `accent-color`), lidos numa thread
  (a interface nunca espera);
- **sem vidro** (ninguém desfoca o fundo): os apps desenham opaco.

Falta:

1. **Sombra** em volta da janela fora do mide (margem transparente + `set_window_geometry`).
2. **Menus nas janelas de conteúdo inteiro** fora do mide (onde ficam: um botão na
   barra de ferramentas?) e menus na barra global do KDE quando houver registro.
3. **Redimensionar pelas bordas** fora do mide (pedir `resize` ao compositor nas bordas).
4. **Ícones**: tema do sistema, com os do mide como reserva.
5. Testar de verdade no GNOME, no KDE e no Hyprland.

## Para a versão 1.0

- Lista do que é público (API estável) e exemplos para cada componente.
- Componentes que faltam do `docs/tahoe.md`: folha (sheet), divisão arrastável,
  inspetor, galeria, estado vazio, seletores de cor e data, avatar, campo de fichas.
- Testes de componentes sem compositor (desenho num buffer).
- Publicação no crates.io.
