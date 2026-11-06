//! Formatos da região (item I1 do mide): números, datas, hora, moeda e o primeiro dia
//! da semana, separados do idioma da interface, como em Ajustes do Sistema → Geral →
//! Idioma e Região do Mac. Alguém com a interface em inglês e a região Brasil vê
//! "1.234,56", "05/11/2026", "14:05" e "R$ 10,00".
//!
//! A região vem de `[locale] formats` (Configurações → Idioma e região) dentro do mide,
//! senão das variáveis do sistema (`LC_ALL`, `LC_NUMERIC`, `LC_TIME`, `LANG`). A tabela
//! de regiões é do próprio mide (sem `libc::localeconv`, que muda o processo inteiro e
//! depende das locales instaladas).
//!
//! Os nomes de meses e dias da semana continuam vindo da tradução (idioma da interface).

use std::sync::Mutex;

/// Ordem dos números da data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    /// 05/11/2026 (Brasil, Europa).
    Dmy,
    /// 11/5/2026 (Estados Unidos).
    Mdy,
    /// 2026-11-05 (Japão, China, Suécia, Canadá).
    Ymd,
}

/// Formatos de uma região.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// Código da região, como "pt_BR".
    pub code: String,
    pub decimal: char,
    /// Separador de milhar.
    pub group: char,
    pub date_order: DateOrder,
    pub date_sep: char,
    /// Relógio de 24 horas.
    pub hour24: bool,
    /// Símbolo da moeda, antes ou depois do número, com ou sem espaço.
    pub currency: &'static str,
    pub currency_prefix: bool,
    pub currency_space: bool,
    /// Primeiro dia da semana nos calendários (0 = domingo, 1 = segunda).
    pub first_weekday: u32,
}

/// Formatos de uma região pelo código ("pt_BR", "pt_BR.UTF-8", "en_US@euro", "de").
/// Região desconhecida: a do idioma; idioma desconhecido (ou "C"/"POSIX"): Estados
/// Unidos com relógio de 24 horas, como o "C" do sistema.
pub fn region_for(code: &str) -> Region {
    let code = code.split(['.', '@']).next().unwrap_or("").replace('-', "_");
    let lang = code.split('_').next().unwrap_or("").to_lowercase();
    let r = |decimal, group, date_order, date_sep, hour24, currency, currency_prefix, currency_space, first_weekday| Region {
        code: code.clone(),
        decimal,
        group,
        date_order,
        date_sep,
        hour24,
        currency,
        currency_prefix,
        currency_space,
        first_weekday,
    };
    use DateOrder::*;
    match code.as_str() {
        "pt_BR" => r(',', '.', Dmy, '/', true, "R$", true, true, 0),
        "pt_PT" => r(',', ' ', Dmy, '/', true, "€", false, true, 1),
        "en_US" => r('.', ',', Mdy, '/', false, "$", true, false, 0),
        "en_GB" | "en_IE" => r('.', ',', Dmy, '/', true, if code == "en_GB" { "£" } else { "€" }, true, false, 1),
        "en_CA" => r('.', ',', Ymd, '-', false, "$", true, false, 0),
        "en_AU" | "en_NZ" => r('.', ',', Dmy, '/', false, "$", true, false, 1),
        "en_IN" => r('.', ',', Dmy, '/', false, "₹", true, false, 0),
        "es_ES" => r(',', '.', Dmy, '/', true, "€", false, true, 1),
        "es_MX" => r('.', ',', Dmy, '/', false, "$", true, false, 0),
        "es_AR" | "es_CL" | "es_CO" | "es_UY" => r(',', '.', Dmy, '/', true, "$", true, true, 1),
        "fr_FR" | "fr_BE" => r(',', ' ', Dmy, '/', true, "€", false, true, 1),
        "fr_CA" => r(',', ' ', Ymd, '-', true, "$", false, true, 0),
        "fr_CH" | "de_CH" | "it_CH" => r('.', '\'', Dmy, '.', true, "CHF", true, true, 1),
        "de_DE" | "de_AT" => r(',', '.', Dmy, '.', true, "€", false, true, 1),
        "it_IT" => r(',', '.', Dmy, '/', true, "€", false, true, 1),
        "nl_NL" => r(',', '.', Dmy, '-', true, "€", true, true, 1),
        "ja_JP" => r('.', ',', Ymd, '/', true, "¥", true, false, 0),
        "zh_CN" => r('.', ',', Ymd, '/', true, "¥", true, false, 1),
        "zh_TW" => r('.', ',', Ymd, '/', false, "$", true, false, 0),
        "ko_KR" => r('.', ',', Ymd, '.', false, "₩", true, false, 0),
        "ru_RU" => r(',', ' ', Dmy, '.', true, "₽", false, true, 1),
        "pl_PL" => r(',', ' ', Dmy, '.', true, "zł", false, true, 1),
        "sv_SE" => r(',', ' ', Ymd, '-', true, "kr", false, true, 1),
        _ => match lang.as_str() {
            "pt" => region_for("pt_BR"),
            "es" => region_for("es_ES"),
            "fr" => region_for("fr_FR"),
            "de" => region_for("de_DE"),
            "it" => region_for("it_IT"),
            "ja" => region_for("ja_JP"),
            "zh" => region_for("zh_CN"),
            "ru" => region_for("ru_RU"),
            "en" => region_for("en_US"),
            _ => Region { code: code.clone(), hour24: true, ..region_for("en_US") },
        },
    }
}

/// Código da região da sessão: a escolha das Configurações no mide, senão a do sistema.
fn session_code() -> String {
    if crate::control_client::on_mide()
        && let Some(crate::settings_file::Value::Text(code)) = crate::settings_file::cached("locale", "formats")
        && !code.is_empty()
    {
        return code;
    }
    ["LC_ALL", "LC_NUMERIC", "LC_TIME", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .unwrap_or_else(|| "C".into())
}

/// Formatos da região da sessão (relidos quando a escolha muda), com o relógio de 12
/// ou 24 horas escolhido à mão (`[locale] clock24`) por cima.
pub fn region() -> Region {
    static CACHE: Mutex<Option<(String, Option<bool>, Region)>> = Mutex::new(None);
    let code = session_code();
    let clock24 = match crate::settings_file::cached("locale", "clock24") {
        Some(crate::settings_file::Value::Bool(b)) if crate::control_client::on_mide() => Some(b),
        _ => None,
    };
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((c, h, r)) = cache.as_ref()
        && *c == code
        && *h == clock24
    {
        return r.clone();
    }
    let mut r = region_for(&code);
    if let Some(h) = clock24 {
        r.hour24 = h;
    }
    *cache = Some((code, clock24, r.clone()));
    r
}

impl Region {
    /// Número com `decimals` casas e o separador de milhar: "1.234,56".
    pub fn number(&self, value: f64, decimals: usize) -> String {
        let text = format!("{:.*}", decimals, value.abs());
        let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
        let mut grouped = String::new();
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                grouped.push(self.group);
            }
            grouped.push(c);
        }
        let sign = if value < 0.0 && text.chars().any(|c| c.is_ascii_digit() && c != '0') { "-" } else { "" };
        if frac.is_empty() { format!("{sign}{grouped}") } else { format!("{sign}{grouped}{}{frac}", self.decimal) }
    }

    /// Hora: "14:05" (24 h) ou "2:05 PM" (12 h); com segundos se pedidos.
    pub fn time(&self, hour: u32, minute: u32, second: Option<u32>) -> String {
        let secs = second.map(|s| format!(":{s:02}")).unwrap_or_default();
        if self.hour24 {
            return format!("{hour:02}:{minute:02}{secs}");
        }
        let h12 = match hour % 12 {
            0 => 12,
            h => h,
        };
        let suffix = if hour < 12 { crate::kit::tr("AM") } else { crate::kit::tr("PM") };
        format!("{h12}:{minute:02}{secs} {suffix}")
    }

    /// Data só com números: "05/11/2026", "11/5/2026", "2026-11-05".
    pub fn date(&self, year: i32, month: u32, day: u32) -> String {
        let s = self.date_sep;
        match self.date_order {
            DateOrder::Dmy => format!("{day:02}{s}{month:02}{s}{year}"),
            DateOrder::Mdy => format!("{month}{s}{day}{s}{year}"),
            DateOrder::Ymd => format!("{year}{s}{month:02}{s}{day:02}"),
        }
    }

    /// Data sem o ano: "05/11", "11/5", "11-05".
    pub fn day_month(&self, month: u32, day: u32) -> String {
        let s = self.date_sep;
        match self.date_order {
            DateOrder::Dmy => format!("{day:02}{s}{month:02}"),
            DateOrder::Mdy => format!("{month}{s}{day}"),
            DateOrder::Ymd => format!("{month:02}{s}{day:02}"),
        }
    }

    /// Valor em dinheiro com o símbolo da região: "R$ 1.234,56", "$1,234.56", "12,00 €".
    pub fn currency(&self, value: f64) -> String {
        let n = self.number(value, 2);
        let space = if self.currency_space { "\u{a0}" } else { "" };
        if self.currency_prefix { format!("{}{space}{n}", self.currency) } else { format!("{n}{space}{}", self.currency) }
    }
}

/// Hora de um instante (`chrono`) no formato da região.
pub fn time_of(t: &impl chrono::Timelike, seconds: bool) -> String {
    region().time(t.hour(), t.minute(), seconds.then(|| t.second()))
}

/// Data de um dia (`chrono`) no formato da região.
pub fn date_of(d: &impl chrono::Datelike) -> String {
    region().date(d.year(), d.month(), d.day())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeros_por_regiao() {
        let br = region_for("pt_BR.UTF-8");
        assert_eq!(br.number(1234.5, 2), "1.234,50");
        assert_eq!(br.number(-1234567.0, 0), "-1.234.567");
        assert_eq!(br.number(-0.001, 2), "0,00");
        assert_eq!(region_for("en_US").number(1234.5, 1), "1,234.5");
        assert_eq!(region_for("de_CH").number(1234.5, 2), "1'234.50");
        assert_eq!(region_for("fr_FR").number(999.0, 0), "999");
    }

    #[test]
    fn datas_e_horas_por_regiao() {
        let br = region_for("pt_BR");
        assert_eq!(br.date(2026, 11, 5), "05/11/2026");
        assert_eq!(br.time(14, 5, None), "14:05");
        let us = region_for("en_US");
        assert_eq!(us.date(2026, 11, 5), "11/5/2026");
        assert_eq!(us.day_month(11, 5), "11/5");
        assert!(us.time(0, 5, Some(9)).starts_with("12:05:09 "));
        assert!(us.time(14, 5, None).starts_with("2:05 "));
        assert_eq!(region_for("ja_JP").date(2026, 1, 2), "2026/01/02");
        assert_eq!(region_for("de_DE").date(2026, 1, 2), "02.01.2026");
    }

    #[test]
    fn moeda_e_semana() {
        assert_eq!(region_for("pt_BR").currency(1234.5), "R$\u{a0}1.234,50");
        assert_eq!(region_for("en_US").currency(3.0), "$3.00");
        assert_eq!(region_for("de_DE").currency(12.0), "12,00\u{a0}€");
        assert_eq!(region_for("pt_BR").first_weekday, 0);
        assert_eq!(region_for("pt_PT").first_weekday, 1);
        // Idioma sem região: a do idioma; "C": números dos EUA com 24 horas.
        assert_eq!(region_for("pt").code, "pt_BR");
        let c = region_for("C");
        assert!(c.hour24 && c.decimal == '.');
    }
}
