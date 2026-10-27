//! Menus do app na barra superior do mide (como no macOS): o app descreve os menus
//! ([`App::menu_bar`](super::App::menu_bar)) e o mide-ui publica pelo D-Bus
//! (`com.canonical.dbusmenu`) e registra no registrar da barra. O clique num item
//! chega ao app como uma ação ([`Ui::menu_action`](super::Ui::menu_action)).
//!
//! O menu "Janela" (Minimizar, Zoom) vem pronto e é tratado pelo próprio mide-ui.

use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
};

use zbus::zvariant::{Array, Dict, OwnedValue, Signature, StructureBuilder, Value};

use super::app::Waker;

/// Um menu da barra (título e itens).
#[derive(Debug, Clone, PartialEq)]
pub struct MenuGroup {
    pub title: String,
    pub items: Vec<MenuItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MenuItem {
    Action {
        /// O que o app recebe no clique.
        action: String,
        label: String,
        /// Atalho mostrado à direita, como o dbusmenu entende: ["Control", "Shift", "t"].
        shortcut: Vec<String>,
        enabled: bool,
        checked: Option<bool>,
    },
    Separator,
    Submenu {
        label: String,
        items: Vec<MenuItem>,
    },
}

impl MenuItem {
    pub fn action(action: &str, label: &str) -> Self {
        MenuItem::Action { action: action.into(), label: label.into(), shortcut: Vec::new(), enabled: true, checked: None }
    }

    /// O mesmo item com o atalho (`"Ctrl+Shift+T"` ou `"Super+T"`).
    pub fn shortcut(mut self, combo: &str) -> Self {
        if let MenuItem::Action { shortcut, .. } = &mut self {
            *shortcut = combo
                .split('+')
                .filter(|k| !k.is_empty())
                .map(|k| match k {
                    "Ctrl" => "Control".to_string(),
                    other if other.chars().count() == 1 => other.to_lowercase(),
                    other => other.to_string(),
                })
                .collect();
        }
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        if let MenuItem::Action { enabled, .. } = &mut self {
            *enabled = on;
        }
        self
    }

    pub fn checked(mut self, on: bool) -> Self {
        if let MenuItem::Action { checked, .. } = &mut self {
            *checked = Some(on);
        }
        self
    }
}

/// Ações do menu "Janela" tratadas pelo mide-ui.
pub const MINIMIZE: &str = "mide-ui:minimizar";
pub const ZOOM: &str = "mide-ui:zoom";

/// Menu "Janela" padrão, com os itens extras do app (se houver) depois.
pub fn window_menu(extra: Vec<MenuItem>) -> MenuGroup {
    let mut items = vec![MenuItem::action(MINIMIZE, "Minimizar").shortcut("Super+H")];
    // No mide, os itens do menu Janela do Mac (o compositor faz cada um); fora dele, só
    // os que a própria janela consegue (minimizar e maximizar).
    if crate::control_client::on_mide() {
        let w = |verb: &str, label: &str| MenuItem::action(&format!("{WINDOW_PREFIX}{verb}"), label);
        items.extend([
            w("fill", "Preencher"),
            w("center", "Centralizar"),
            MenuItem::Separator,
            MenuItem::Submenu {
                label: "Mover e redimensionar".into(),
                items: vec![
                    w("left", "Metade esquerda"),
                    w("right", "Metade direita"),
                    w("top", "Metade de cima"),
                    w("bottom", "Metade de baixo"),
                    MenuItem::Separator,
                    w("top-left", "Canto superior esquerdo"),
                    w("top-right", "Canto superior direito"),
                    w("bottom-left", "Canto inferior esquerdo"),
                    w("bottom-right", "Canto inferior direito"),
                    MenuItem::Separator,
                    w("restore", "Voltar ao tamanho anterior"),
                ],
            },
            MenuItem::Submenu {
                label: "Lado a lado na tela".into(),
                items: vec![w("left", "À esquerda da tela"), w("right", "À direita da tela")],
            },
            MenuItem::Separator,
            w("minimize-all", "Minimizar todas"),
            w("front", "Trazer todas para a frente"),
        ]);
    } else {
        items.push(MenuItem::action(ZOOM, "Zoom"));
    }
    if !extra.is_empty() {
        items.push(MenuItem::Separator);
        items.extend(extra);
    }
    MenuGroup { title: "Janela".into(), items }
}

/// Começo das ações do menu Janela feitas pelo compositor (`mide-ui:janela:fill`...).
pub const WINDOW_PREFIX: &str = "mide-ui:janela:";

/// Ação do menu Janela que é do compositor: manda `window <verbo>` para a janela em foco
/// (numa thread: o desenho não espera a resposta). Devolve se a ação era dessas.
pub fn run_window_action(action: &str) -> bool {
    let Some(verb) = action.strip_prefix(WINDOW_PREFIX) else {
        return false;
    };
    let verb = verb.to_string();
    std::thread::spawn(move || {
        let _ = crate::control_client::ask_compositor(&format!("window {verb}"));
    });
    true
}

/// Árvore do menu como o dbusmenu pede: números estáveis (pela posição) para cada
/// item e a ação de cada número.
#[derive(Debug, Default)]
struct Tree {
    groups: Vec<MenuGroup>,
    actions: HashMap<i32, String>,
}

impl Tree {
    fn new(groups: Vec<MenuGroup>) -> Self {
        let mut actions = HashMap::new();
        let mut next = 1;
        fn number(items: &[MenuItem], next: &mut i32, actions: &mut HashMap<i32, String>) {
            for item in items {
                let id = *next;
                *next += 1;
                match item {
                    MenuItem::Action { action, .. } => {
                        actions.insert(id, action.clone());
                    }
                    MenuItem::Submenu { items, .. } => number(items, next, actions),
                    MenuItem::Separator => {}
                }
            }
        }
        for group in &groups {
            next += 1;
            number(&group.items, &mut next, &mut actions);
        }
        Self { groups, actions }
    }

    /// O nó `id` (0 = raiz) com tudo abaixo dele.
    fn node(&self, id: i32) -> Value<'static> {
        let mut next = 1;
        let mut found = None;
        let mut groups = Vec::new();
        for group in &self.groups {
            let gid = next;
            next += 1;
            let children = build(&group.items, &mut next, id, &mut found);
            let node = node(gid, &group.title, props_submenu(), children);
            if gid == id {
                found = Some(node.try_clone().expect("nó"));
            }
            groups.push(node);
        }
        if id != 0 {
            if let Some(found) = found {
                return found;
            }
        }
        node(0, "", props_submenu(), groups)
    }
}

fn build(items: &[MenuItem], next: &mut i32, wanted: i32, found: &mut Option<Value<'static>>) -> Vec<Value<'static>> {
    let mut out = Vec::new();
    for item in items {
        let id = *next;
        *next += 1;
        let value = match item {
            MenuItem::Separator => {
                let mut props = Dict::new(&Signature::Str, &Signature::Variant);
                insert(&mut props, "type", Value::from("separator".to_string()));
                node_with(id, props, Vec::new())
            }
            MenuItem::Action { label, shortcut, enabled, checked, .. } => {
                let mut props = Dict::new(&Signature::Str, &Signature::Variant);
                insert(&mut props, "label", Value::from(label.clone()));
                insert(&mut props, "enabled", Value::from(*enabled));
                if let Some(on) = checked {
                    insert(&mut props, "toggle-type", Value::from("checkmark".to_string()));
                    insert(&mut props, "toggle-state", Value::from(*on as i32));
                }
                if !shortcut.is_empty() {
                    let mut keys = Array::new(&Signature::Str);
                    for k in shortcut {
                        let _ = keys.append(Value::from(k.clone()));
                    }
                    let mut combos = Array::new(<Vec<String> as zbus::zvariant::Type>::SIGNATURE);
                    let _ = combos.append(Value::Array(keys));
                    insert(&mut props, "shortcut", Value::Array(combos));
                }
                node_with(id, props, Vec::new())
            }
            MenuItem::Submenu { label, items } => {
                let children = build(items, next, wanted, found);
                let mut props = props_submenu();
                insert(&mut props, "label", Value::from(label.clone()));
                node_with(id, props, children)
            }
        };
        if id == wanted {
            *found = Some(value.try_clone().expect("nó"));
        }
        out.push(value);
    }
    out
}

fn insert(props: &mut Dict<'static, 'static>, key: &str, value: Value<'static>) {
    let _ = props.append(Value::from(key.to_string()), Value::Value(Box::new(value)));
}

fn props_submenu() -> Dict<'static, 'static> {
    let mut props = Dict::new(&Signature::Str, &Signature::Variant);
    insert(&mut props, "children-display", Value::from("submenu".to_string()));
    props
}

fn node(id: i32, label: &str, mut props: Dict<'static, 'static>, children: Vec<Value<'static>>) -> Value<'static> {
    if !label.is_empty() {
        insert(&mut props, "label", Value::from(label.to_string()));
    }
    node_with(id, props, children)
}

fn node_with(id: i32, props: Dict<'static, 'static>, children: Vec<Value<'static>>) -> Value<'static> {
    let mut array = Array::new(&Signature::Variant);
    for child in children {
        let _ = array.append(Value::Value(Box::new(child)));
    }
    StructureBuilder::new()
        .append_field(Value::I32(id))
        .append_field(Value::Dict(props))
        .append_field(Value::Array(array))
        .build()
        .expect("nó do menu")
        .into()
}

struct Service {
    tree: Arc<Mutex<Tree>>,
    clicks: mpsc::Sender<String>,
    waker: Option<Waker>,
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Service {
    fn get_layout(&self, parent_id: i32, _depth: i32, _props: Vec<String>) -> (u32, OwnedValue) {
        let root = self.tree.lock().map(|t| t.node(parent_id)).unwrap_or_else(|_| node(0, "", props_submenu(), Vec::new()));
        (1, OwnedValue::try_from(root).unwrap_or_else(|_| OwnedValue::from(0i32)))
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn event(&self, id: i32, event_id: String, _data: OwnedValue, _timestamp: u32) {
        if event_id != "clicked" {
            return;
        }
        let action = self.tree.lock().ok().and_then(|t| t.actions.get(&id).cloned());
        if let Some(action) = action {
            let _ = self.clicks.send(action);
            if let Some(w) = &self.waker {
                w.wake();
            }
        }
    }

    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "normal".into()
    }

    fn get_group_properties(&self, _ids: Vec<i32>, _props: Vec<String>) -> Vec<(i32, HashMap<String, OwnedValue>)> {
        Vec::new()
    }
}

/// Menus publicados: a árvore (trocada quando o app muda os menus) e os cliques.
pub struct Published {
    tree: Arc<Mutex<Tree>>,
    current: Vec<MenuGroup>,
    pub clicks: mpsc::Receiver<String>,
}

impl Published {
    /// Publica no barramento numa thread (registra de novo quando a barra reinicia).
    pub fn start(groups: Vec<MenuGroup>, waker: Option<Waker>) -> Option<Self> {
        let tree = Arc::new(Mutex::new(Tree::new(groups.clone())));
        let (tx, clicks) = mpsc::channel();
        let service = Service { tree: tree.clone(), clicks: tx, waker };
        std::thread::Builder::new()
            .name("menus do app".into())
            .spawn(move || {
                let Ok(conn) = zbus::blocking::connection::Builder::session().and_then(|b| b.serve_at("/MenuBar", service)).and_then(|b| b.build()) else {
                    tracing::debug!("menus do app: sem barramento da sessão");
                    return;
                };
                let window = std::process::id();
                let register = || -> bool {
                    let Ok(path) = zbus::zvariant::ObjectPath::try_from("/MenuBar") else {
                        return false;
                    };
                    zbus::blocking::Proxy::new(&conn, "com.canonical.AppMenu.Registrar", "/com/canonical/AppMenu/Registrar", "com.canonical.AppMenu.Registrar")
                        .and_then(|p| p.call::<_, _, ()>("RegisterWindow", &(window, &path)))
                        .is_ok()
                };
                // A barra pode ainda não estar pronta: tenta por alguns segundos.
                for _ in 0..30 {
                    if register() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
                // A barra reiniciou (atualizar sem deslogar): registra de novo.
                let Ok(dbus) = zbus::blocking::fdo::DBusProxy::new(&conn) else {
                    return;
                };
                let Ok(changes) = dbus.receive_name_owner_changed_with_args(&[(0, "com.canonical.AppMenu.Registrar")]) else {
                    return;
                };
                for change in changes {
                    if change.args().is_ok_and(|a| a.new_owner().is_some()) {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        register();
                    }
                }
            })
            .ok()?;
        Some(Self { tree, current: groups, clicks })
    }

    /// Troca os menus (se mudaram).
    pub fn update(&mut self, groups: Vec<MenuGroup>) {
        if groups == self.current {
            return;
        }
        if let Ok(mut tree) = self.tree.lock() {
            *tree = Tree::new(groups.clone());
        }
        self.current = groups;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups() -> Vec<MenuGroup> {
        vec![
            MenuGroup { title: "Shell".into(), items: vec![MenuItem::action("nova-aba", "Nova aba").shortcut("Ctrl+Shift+T"), MenuItem::Separator, MenuItem::action("fechar", "Fechar")] },
            window_menu(vec![]),
        ]
    }

    #[test]
    fn numeros_estaveis_e_acoes() {
        let tree = Tree::new(groups());
        // Shell = 1, Nova aba = 2, separador = 3, Fechar = 4, Janela = 5, Minimizar = 6;
        // depois, Preencher (no mide) ou Zoom (fora dele).
        assert_eq!(tree.actions.get(&2).map(String::as_str), Some("nova-aba"));
        assert_eq!(tree.actions.get(&4).map(String::as_str), Some("fechar"));
        assert_eq!(tree.actions.get(&6).map(String::as_str), Some(MINIMIZE));
        let next = tree.actions.get(&7).map(String::as_str).unwrap_or_default();
        assert!(next == ZOOM || next == format!("{WINDOW_PREFIX}fill"), "{next}");
        assert!(!tree.actions.contains_key(&3));
    }

    #[test]
    fn a_barra_le_o_que_publicamos() {
        let tree = Tree::new(groups());
        let root = crate::appmenu::parse_for_test(&tree.node(0)).unwrap();
        let titles: Vec<&str> = root.children.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(titles, ["Shell", "Janela"]);
        let shell = crate::appmenu::parse_for_test(&tree.node(1)).unwrap();
        assert_eq!(shell.children[0].label, "Nova aba");
        assert_eq!(shell.children[0].shortcut, "Ctrl+Shift+T");
        assert!(shell.children[1].separator);
    }

    #[test]
    fn atalhos_do_mac_com_simbolos() {
        let groups = vec![MenuGroup {
            title: "Shell".into(),
            items: vec![MenuItem::action("a", "A").shortcut("Super+Shift+D"), MenuItem::action("b", "B").shortcut("Ctrl+Page_Down")],
        }];
        let shell = crate::appmenu::parse_for_test(&Tree::new(groups).node(1)).unwrap();
        assert_eq!(shell.children[0].shortcut, "⌘⇧D");
        assert_eq!(shell.children[1].shortcut, "Ctrl+PgDn");
    }
}
