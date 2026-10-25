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

1. **Barra de título própria** quando o compositor não desenha a dele (GNOME e outros
   sem decoração do servidor): negociar `xdg-decoration` e, sem ela, desenhar luzes,
   título e arrasto em toda janela, não só nas de conteúdo inteiro.
2. **Cantos arredondados e sombra pelo app** quando o compositor não arredonda (hoje o
   mide faz isso por `window_style`).
3. **Tema e cor de destaque pelo portal** (`org.freedesktop.appearance.color-scheme` e
   `accent-color` do `xdg-desktop-portal`) fora do mide; dentro dele, a configuração do
   mide.
4. **Vidro**: desfoque pelo protocolo do mide quando existir; fora, fundo translúcido
   sem desfoque (ou `ext-background-effect` onde houver).
5. **Menus**: na barra superior quando houver o registro de menus (mide, KDE com menu
   global); senão, menus dentro da janela.
6. **Ícones**: tema do sistema, com os do mide como reserva.

## Para a versão 1.0

- Lista do que é público (API estável) e exemplos para cada componente.
- Componentes que faltam do `docs/tahoe.md`: folha (sheet), divisão arrastável,
  inspetor, galeria, estado vazio, seletores de cor e data, avatar, campo de fichas.
- Testes de componentes sem compositor (desenho num buffer).
- Publicação no crates.io.
