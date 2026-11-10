//! Teste da tela de bloqueio no X11 (`Window::lock` no `kit::x11_app`): tela inteira por
//! cima de tudo, teclado e ponteiro presos, teclas com Shift, e saída com `ui.unlock()`
//! (o `q` desbloqueia, como a senha aceita no mide-lock). Imprime `PRONTO`, `TECLA` e
//! `DESBLOQUEIO`; o script `tests/x11/bloqueio.sh` confere de fora.

use std::io::Write;

use mide_ui::kit::{self, App, Ui, Window};

struct Teste;

impl App for Teste {
    fn view(&mut self, ui: &mut Ui) {
        let input = ui.input().clone();
        if !input.text.is_empty() {
            println!("TECLA {:?} shift={}", input.text, input.shift);
            let _ = std::io::stdout().flush();
        }
        if input.text.contains('q') {
            println!("DESBLOQUEIO");
            let _ = std::io::stdout().flush();
            ui.unlock();
        }
    }
}

fn main() {
    println!("PRONTO");
    let _ = std::io::stdout().flush();
    if let Err(e) = kit::run(Window::new("Bloqueio teste").app_id("conf-bloqueio").lock(), Teste) {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}
