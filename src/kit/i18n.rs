//! Tradução da interface: inglês como base, catálogos `.po` por idioma.
//!
//! Os textos da interface são escritos em inglês no código e passam por [`tr`] (ou pela
//! macro [`tr!`](crate::tr), que também preenche `{nome}`), [`trn`] (plural) e [`trc`]
//! (com contexto, para palavras iguais com sentidos diferentes). O idioma vem da sessão
//! (`MIDE_LANG`, depois `LC_ALL`, `LC_MESSAGES`, `LANG` e `LANGUAGE`); sem tradução, o
//! texto em inglês aparece. O mide-ui traz o catálogo dos próprios textos
//! (`locales/pt_BR.po`); os apps registram os deles com [`add_catalog`].
//!
//! Os catálogos seguem o formato do gettext (`msgid`/`msgstr`, `msgid_plural` com
//! `msgstr[N]`, `msgctxt`), lido aqui mesmo: qualquer editor de `.po` (Poedit, Weblate)
//! serve para traduzir. Entradas vazias ou marcadas `fuzzy` ficam de fora.

use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::Display,
    sync::{OnceLock, RwLock},
};

/// Separador do gettext entre o contexto e o texto (`msgctxt`).
const CONTEXT_SEP: char = '\u{4}';

/// Traduções de um idioma: texto em inglês (com o contexto na frente, se houver) →
/// formas traduzidas (uma; ou uma por forma de plural).
type Catalog = HashMap<&'static str, Vec<&'static str>>;

struct Registry {
    /// Idioma da sessão, já normalizado ("pt_BR", "en").
    language: String,
    catalogs: HashMap<String, Catalog>,
}

fn registry() -> &'static RwLock<Registry> {
    static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = Registry { language: session_language(), catalogs: HashMap::new() };
        load(&mut registry, "pt_BR", include_str!("../../locales/pt_BR.po"));
        RwLock::new(registry)
    })
}

/// Idioma da interface pelas variáveis da sessão, normalizado ("pt_BR.UTF-8" → "pt_BR";
/// "C" e "POSIX" → "en").
pub fn session_language() -> String {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let raw = var("MIDE_LANG")
        .or_else(|| var("LC_ALL"))
        .or_else(|| var("LC_MESSAGES"))
        .or_else(|| var("LANG"))
        .or_else(|| var("LANGUAGE").map(|l| l.split(':').next().unwrap_or("").to_string()))
        .unwrap_or_default();
    normalize(&raw)
}

fn normalize(raw: &str) -> String {
    let code = raw.split(['.', '@']).next().unwrap_or("").replace('-', "_");
    if code.is_empty() || code == "C" || code == "POSIX" { "en".into() } else { code }
}

/// Idioma em uso pela interface.
pub fn language() -> String {
    registry().read().map(|r| r.language.clone()).unwrap_or_else(|_| "en".into())
}

/// Troca o idioma da interface (testes, ou a escolha nas Configurações valendo na hora).
pub fn set_language(language: &str) {
    if let Ok(mut r) = registry().write() {
        r.language = normalize(language);
    }
}

/// Junta um catálogo `.po` ao idioma `language` ("pt_BR"). Entradas repetidas: vale a
/// última. Os apps chamam no começo, com o texto embutido (`include_str!`).
pub fn add_catalog(language: &str, po: &str) {
    if let Ok(mut r) = registry().write() {
        load(&mut r, language, po);
    }
}

fn load(registry: &mut Registry, language: &str, po: &str) {
    let catalog = registry.catalogs.entry(normalize(language)).or_default();
    for (key, forms) in parse_po(po) {
        let key: &'static str = Box::leak(key.into_boxed_str());
        let forms = forms.into_iter().map(|f| &*Box::leak(f.into_boxed_str())).collect();
        catalog.insert(key, forms);
    }
}

/// Procura `key` no idioma da sessão e, sem ele, no idioma sem a região ("pt_BR" → "pt").
fn lookup(key: &str) -> Option<Vec<&'static str>> {
    let r = registry().read().ok()?;
    let lang = r.language.as_str();
    let base = lang.split('_').next().unwrap_or(lang);
    [lang, base].iter().find_map(|l| r.catalogs.get(*l)?.get(key).cloned())
}

/// O catálogo do idioma em uso tem `text` (com `context`, se houver)? Para os testes
/// que conferem se todo texto do app foi traduzido.
pub fn is_translated(context: Option<&str>, text: &str) -> bool {
    match context {
        Some(c) => lookup(&format!("{c}{CONTEXT_SEP}{text}")).is_some(),
        None => lookup(text).is_some(),
    }
}

/// Texto da interface no idioma da sessão. `text` é o texto em inglês.
pub fn tr(text: &str) -> Cow<'_, str> {
    match lookup(text).and_then(|f| f.first().copied()) {
        Some(t) => Cow::Borrowed(t),
        None => Cow::Borrowed(text),
    }
}

/// Como [`tr`], com contexto (`msgctxt`): o mesmo texto em inglês com traduções
/// diferentes ("Open" do menu e "Open" de estado, por exemplo).
pub fn trc<'a>(context: &str, text: &'a str) -> Cow<'a, str> {
    match lookup(&format!("{context}{CONTEXT_SEP}{text}")).and_then(|f| f.first().copied()) {
        Some(t) => Cow::Borrowed(t),
        None => Cow::Borrowed(text),
    }
}

/// Plural: `singular` ou `plural` (em inglês) conforme `n`, traduzido, com `{n}`
/// trocado pelo número.
pub fn trn(singular: &str, plural: &str, n: u64) -> String {
    let lang = language();
    let chosen = match lookup(singular) {
        Some(forms) if !forms.is_empty() => forms[plural_index(&lang, n).min(forms.len() - 1)],
        _ => {
            if n == 1 {
                singular
            } else {
                plural
            }
        }
    };
    chosen.replace("{n}", &n.to_string())
}

/// Qual forma de plural do gettext usar (regras dos idiomas mais comuns; o resto, a do
/// inglês).
fn plural_index(language: &str, n: u64) -> usize {
    let base = language.split('_').next().unwrap_or(language);
    match (language, base) {
        // Português do Brasil e francês: 0 e 1 no singular.
        ("pt_BR", _) | (_, "fr") => usize::from(n > 1),
        (_, "ja" | "zh" | "ko" | "vi" | "th") => 0,
        _ => usize::from(n != 1),
    }
}

/// Troca `{nome}` em `text` pelos valores de `args`. Usado pela macro [`tr!`](crate::tr).
pub fn fill(text: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = text.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}

/// Lê um `.po`: pares (chave, formas). A chave é o `msgid`, com `msgctxt` + `\u{4}` na
/// frente quando há contexto.
fn parse_po(po: &str) -> Vec<(String, Vec<String>)> {
    #[derive(Default)]
    struct Entry {
        context: Option<String>,
        id: String,
        forms: Vec<String>,
        fuzzy: bool,
        /// Já leu um `msgid` (o do cabeçalho é vazio, então o texto não basta).
        has_id: bool,
    }
    enum Field {
        None,
        Context,
        Id,
        Plural,
        Str(usize),
    }
    fn finish(entry: &mut Entry, out: &mut Vec<(String, Vec<String>)>) {
        let done = std::mem::take(entry);
        if done.id.is_empty() || done.fuzzy || done.forms.iter().all(String::is_empty) {
            return;
        }
        let key = match done.context {
            Some(c) => format!("{c}{CONTEXT_SEP}{}", done.id),
            None => done.id,
        };
        out.push((key, done.forms));
    }
    let mut out = Vec::new();
    let mut entry = Entry::default();
    let mut field = Field::None;
    for line in po.lines().map(str::trim) {
        if line.is_empty() {
            continue;
        }
        // Começo de outra entrada: comentário ou contexto depois de um msgstr.
        let starts_new = line.starts_with('#') || line.starts_with("msgctxt") || line.starts_with("msgid ");
        if starts_new && entry.has_id && !entry.forms.is_empty() {
            finish(&mut entry, &mut out);
            field = Field::None;
        }
        if let Some(flags) = line.strip_prefix("#,") {
            entry.fuzzy |= flags.split(',').any(|f| f.trim() == "fuzzy");
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let (keyword, rest) = match line.find('"') {
            Some(i) => (line[..i].trim(), &line[i..]),
            None => continue,
        };
        let value = unquote(rest);
        match keyword {
            "msgctxt" => {
                entry.context = Some(value);
                field = Field::Context;
            }
            "msgid" => {
                entry.id = value;
                entry.has_id = true;
                field = Field::Id;
            }
            "msgid_plural" => field = Field::Plural,
            "msgstr" => {
                entry.forms = vec![value];
                field = Field::Str(0);
            }
            k if k.starts_with("msgstr[") => {
                let i: usize = k.trim_start_matches("msgstr[").trim_end_matches(']').parse().unwrap_or(0);
                if entry.forms.len() <= i {
                    entry.forms.resize(i + 1, String::new());
                }
                entry.forms[i] = value;
                field = Field::Str(i);
            }
            // Linha só com texto: continuação do campo anterior.
            "" => match field {
                Field::Context => {
                    if let Some(c) = entry.context.as_mut() {
                        c.push_str(&value);
                    }
                }
                Field::Id => entry.id.push_str(&value),
                Field::Str(i) => entry.forms[i].push_str(&value),
                Field::Plural | Field::None => {}
            },
            _ => {}
        }
    }
    finish(&mut entry, &mut out);
    out
}

/// Tira as aspas e desfaz os escapes (`\n`, `\t`, `\"`, `\\`).
fn unquote(s: &str) -> String {
    let inner = s.trim().strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Texto da interface traduzido, com `{nome}` preenchido:
/// `tr!("Delete")`, `tr!("Move “{name}” to the Trash?", name = file)`.
/// Sem argumentos dá um `Cow<str>`; com argumentos, um `String`.
#[macro_export]
macro_rules! tr {
    ($text:expr $(,)?) => {
        $crate::kit::tr($text)
    };
    ($text:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::kit::i18n::fill(&$crate::kit::tr($text), &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const PO: &str = r#"
# Comentário
msgid ""
msgstr "Content-Type: text/plain; charset=UTF-8\n"

msgid "Open"
msgstr "Abrir"

msgctxt "state"
msgid "Open"
msgstr "Aberto"

msgid "{n} item"
msgid_plural "{n} items"
msgstr[0] "{n} item"
msgstr[1] "{n} itens"

#, fuzzy
msgid "Close"
msgstr "Fechar errado"

msgid "Two\n"
"lines"
msgstr "Duas\n"
"linhas"

msgid "Untranslated"
msgstr ""
"#;

    #[test]
    fn le_o_po() {
        let entries: HashMap<String, Vec<String>> = parse_po(PO).into_iter().collect();
        assert_eq!(entries["Open"], vec!["Abrir"]);
        assert_eq!(entries[&format!("state{CONTEXT_SEP}Open")], vec!["Aberto"]);
        assert_eq!(entries["{n} item"], vec!["{n} item", "{n} itens"]);
        assert_eq!(entries["Two\nlines"], vec!["Duas\nlinhas"]);
        assert!(!entries.contains_key("Close"), "fuzzy fica de fora");
        assert!(!entries.contains_key("Untranslated"));
        assert!(!entries.contains_key(""), "cabeçalho fica de fora");
    }

    #[test]
    fn normaliza_o_idioma() {
        assert_eq!(normalize("pt_BR.UTF-8"), "pt_BR");
        assert_eq!(normalize("de_DE@euro"), "de_DE");
        assert_eq!(normalize("C"), "en");
        assert_eq!(normalize(""), "en");
    }

    #[test]
    fn plural_por_idioma() {
        assert_eq!(plural_index("pt_BR", 0), 0);
        assert_eq!(plural_index("pt_BR", 2), 1);
        assert_eq!(plural_index("en", 0), 1);
        assert_eq!(plural_index("en", 1), 0);
    }

    #[test]
    fn preenche_os_nomes() {
        assert_eq!(fill("Delete “{name}”? ({n})", &[("name", &"a.txt"), ("n", &3)]), "Delete “a.txt”? (3)");
    }
}
