//! regole.rs — **N2 (la parte che decide) e N3 (l'intento)**, WO-01 §3 e §4.
//!
//! Qui non si richiama nessuno: si risponde a una domanda sola, *questa finestra
//! e' in pista?*, e la risposta ha **tre** valori, non due. L'incerto e' il terzo,
//! ed e' il valore piu' importante del file: una finestra che le regole non
//! conoscono **non genera richiami**. Il WO lo dice due volte, e la seconda con
//! un numero (WO-05: precisione sul "fuori" almeno 95%) — *un falso richiamo
//! costa piu' di uno mancato*, perche' il primo ti insegna a ignorare la cintura.
//!
//! # L'ordine delle regole, e perche' e' questo
//!
//! 1. **processo in pista** → in pista. Un editor e' un editor, sempre.
//! 2. **processo fuori pista** → fuori pista, e **l'intento non lo sblocca**.
//! 3. **parola in pista nel titolo** → in pista.
//! 4. **parola fuori pista nel titolo** → fuori pista, *salvo* una parola
//!    dell'intento nel titolo: allora e' in pista.
//! 5. altrimenti → **incerto**.
//!
//! Il punto 4 e' il §4 del WO: "rust" nel titolo di YouTube conta come in pista.
//! Il punto 2 e' il suo limite, ed e' una scelta: se l'intento sbloccasse anche
//! i processi, una canzone intitolata *Rust* renderebbe Spotify un ambiente di
//! studio. L'intento sblocca **i titoli ambigui**, non le applicazioni.
//!
//! Il punto 1 prima del 2 significa che, se una regola compare in tutte e due le
//! liste, vince **in pista**. E' la stessa disciplina dell'incerto: fra il rischio
//! di tacere e quello di richiamare a torto, si sceglie di tacere.

use serde::{Deserialize, Serialize};

use crate::finestra::Finestra;

/// La versione dello schema di `rules.toml`. Sale quando un campo cambia
/// significato, non quando se ne aggiunge uno: i campi nuovi hanno un default.
pub const VERSIONE_REGOLE: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdetto {
    InPista,
    FuoriPista,
    /// Le regole non sanno. Non genera richiami, ma **va a registro**: e' il
    /// materiale con cui si scrivono le regole della settimana dopo, ed e' il
    /// campione etichettato che servira' al classificatore locale (WO-05).
    Incerto,
}

impl Verdetto {
    pub fn come_stringa(self) -> &'static str {
        match self {
            Verdetto::InPista => "in_pista",
            Verdetto::FuoriPista => "fuori_pista",
            Verdetto::Incerto => "incerto",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Regole {
    #[serde(default = "versione_default")]
    pub versione: u32,
    #[serde(default)]
    pub soglie: Soglie,
    #[serde(default)]
    pub in_pista: Lista,
    #[serde(default)]
    pub fuori_pista: Lista,
}

fn versione_default() -> u32 {
    VERSIONE_REGOLE
}

/// Le soglie dell'escalation (WO-01 §5), in secondi. Stanno in config perche'
/// il WO lo chiede: sono il primo numero che si vorra' cambiare dopo la prima
/// sessione vera.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Soglie {
    /// Ogni quanto si guarda la finestra attiva. Entra nell'accettazione n. 1:
    /// il ritardo massimo dal cambio finestra al suono e' soglia + questo.
    pub intervallo_ms: u64,
    pub suono_s: u64,
    pub frase_s: u64,
    pub frase_dura_s: u64,
    /// Ogni quanto si ripete, dal livello 3 in poi.
    pub ripetizione_s: u64,
}

impl Default for Soglie {
    fn default() -> Self {
        Self { intervallo_ms: 1000, suono_s: 10, frase_s: 45, frase_dura_s: 120, ripetizione_s: 60 }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Lista {
    #[serde(default)]
    pub processi: Vec<String>,
    #[serde(default)]
    pub titolo_contiene: Vec<String>,
}

impl Default for Regole {
    fn default() -> Self {
        toml::from_str(REGOLE_DEFAULT).expect("le regole di default devono essere TOML valido")
    }
}

impl Regole {
    /// Carica le regole da un file. Un file che non c'e' non e' un errore per
    /// il chiamante: e' il primo avvio, e allora valgono i default.
    pub fn da_file(path: &std::path::Path) -> Result<Self, String> {
        let testo = std::fs::read_to_string(path)
            .map_err(|e| format!("non riesco a leggere {}: {e}", path.display()))?;
        Self::da_testo(&testo)
    }

    pub fn da_testo(testo: &str) -> Result<Self, String> {
        let r: Regole = toml::from_str(testo).map_err(|e| format!("rules.toml non valido: {e}"))?;
        if r.versione != VERSIONE_REGOLE {
            return Err(format!(
                "rules.toml e' alla versione {} e questo binario capisce la {VERSIONE_REGOLE}. \
                 Nessuna migrazione automatica: rinomina il file e lascia che `init` ne scriva uno nuovo.",
                r.versione
            ));
        }
        Ok(r)
    }

    /// Il verdetto su una finestra, dato l'intento della sessione.
    pub fn classifica(&self, f: &Finestra, intento: &Intento) -> Verdetto {
        let processo = f.processo.to_lowercase();
        let titolo = f.titolo.to_lowercase();

        if self.in_pista.processi.iter().any(|p| combacia(&p.to_lowercase(), &processo)) {
            return Verdetto::InPista;
        }
        if self.fuori_pista.processi.iter().any(|p| combacia(&p.to_lowercase(), &processo)) {
            return Verdetto::FuoriPista;
        }
        if self.in_pista.titolo_contiene.iter().any(|w| titolo.contains(&w.to_lowercase())) {
            return Verdetto::InPista;
        }
        if self.fuori_pista.titolo_contiene.iter().any(|w| titolo.contains(&w.to_lowercase())) {
            // §4: le parole dell'intento sbloccano i titoli ambigui.
            if intento.sblocca(&titolo) {
                return Verdetto::InPista;
            }
            return Verdetto::FuoriPista;
        }
        Verdetto::Incerto
    }
}

/// Confronto con `*` come jolly (`Godot_v4*.exe`), a caratteri, senza regex.
///
/// Gia' in minuscolo tutti e due: lo fa il chiamante una volta per tick invece
/// che una volta per regola.
pub fn combacia(pattern: &str, valore: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == valore;
    }
    let pezzi: Vec<&str> = pattern.split('*').collect();
    let mut resto = valore;

    // Il primo pezzo e' ancorato all'inizio, l'ultimo alla fine: "a*b" non deve
    // accettare "xaybz".
    if let Some(primo) = pezzi.first() {
        match resto.strip_prefix(primo) {
            Some(r) => resto = r,
            None => return false,
        }
    }
    if pezzi.len() > 1 {
        if let Some(ultimo) = pezzi.last() {
            if resto.len() < ultimo.len() || !resto.ends_with(ultimo) {
                return false;
            }
            resto = &resto[..resto.len() - ultimo.len()];
        }
    }
    for pezzo in pezzi.iter().skip(1).take(pezzi.len().saturating_sub(2)) {
        match resto.find(pezzo) {
            Some(i) => resto = &resto[i + pezzo.len()..],
            None => return false,
        }
    }
    true
}

/// **N3 — l'intento di sessione.** All'avvio scrivi cosa fai; da quella frase
/// escono le parole che sbloccano i titoli ambigui.
#[derive(Debug, Clone, Default)]
pub struct Intento {
    pub testo: String,
    pub parole: Vec<String>,
}

/// Parole che in italiano non dicono niente su cosa stai facendo. Senza questa
/// lista, un intento come "faccio le cose per l'esame" sbloccherebbe qualunque
/// titolo contenga "cose".
const VUOTE: &[&str] = &[
    "devo", "fare", "faccio", "cosa", "cose", "anche", "come", "dopo", "prima", "questo",
    "questa", "quello", "quella", "della", "delle", "dello", "degli", "dalla", "nella", "nelle",
    "sulla", "sulle", "oggi", "adesso", "molto", "roba", "lavoro", "studio", "studiare",
    "lavorare", "provare", "provo", "capire", "capisco", "vedere", "sistemare", "finire",
    "continuo", "continuare", "mettere", "scrivere", "leggere", "guardare", "sono", "stasera",
];

impl Intento {
    /// Dalla frase dell'intento alle parole che contano.
    ///
    /// Regola: almeno 4 caratteri e non nell'elenco delle vuote. Quattro e non
    /// tre perche' "rust" e' la parola che il §4 cita come esempio, e tre
    /// lascerebbe passare "per", "con", "dei".
    ///
    /// `extra` sono le parole date a mano con `--parole`: entrano **sempre**,
    /// anche corte (`wgpu`, `elo`, `api`), perche' le hai scelte tu.
    pub fn nuovo(testo: &str, extra: &[String]) -> Self {
        let mut parole: Vec<String> = testo
            .split(|c: char| !c.is_alphanumeric())
            .map(|w| w.trim().to_lowercase())
            .filter(|w| w.chars().count() >= 4 && !VUOTE.contains(&w.as_str()))
            .collect();
        for e in extra {
            let e = e.trim().to_lowercase();
            if !e.is_empty() {
                parole.push(e);
            }
        }
        parole.sort();
        parole.dedup();
        Self { testo: testo.trim().to_string(), parole }
    }

    /// Il titolo (gia' in minuscolo) contiene una parola dell'intento?
    pub fn sblocca(&self, titolo_minuscolo: &str) -> bool {
        self.parole.iter().any(|p| titolo_minuscolo.contains(p.as_str()))
    }
}

/// Le regole che `init` scrive al primo avvio. Sono un punto di partenza
/// onesto, non una verita': la prima sessione vera le cambia.
pub const REGOLE_DEFAULT: &str = r####"# rules.toml — le regole della cintura (WO-01 §3)
#
# Tre cose da sapere prima di metterci le mani:
#   1. Cio' che non e' in nessuna lista e' INCERTO, e l'incerto non richiama.
#      Aggiungere una riga qui e' l'unico modo di farsi richiamare di piu'.
#   2. Il titolo del browser contiene gia' il titolo della scheda: le parole
#      sotto `titolo_contiene` lavorano su quello.
#   3. Le parole del tuo intento di sessione sbloccano i titoli di
#      `fuori_pista.titolo_contiene`, ma non i processi di `fuori_pista.processi`.
#
# Il confronto e' sempre senza maiuscole. Nei processi `*` e' un jolly.

versione = 1

[soglie]
intervallo_ms  = 1000   # ogni quanto si guarda la finestra attiva
suono_s        = 10     # oltre: un suono breve
frase_s        = 45     # oltre: una frase registrata
frase_dura_s   = 120    # oltre: una frase piu' dura...
ripetizione_s  = 60     # ...ripetuta ogni tanti secondi, finche' non torni

[in_pista]
processi = [
    "Code.exe", "devenv.exe", "rustrover64.exe", "idea64.exe", "clion64.exe",
    "WindowsTerminal.exe", "pwsh.exe", "powershell.exe", "cmd.exe", "wezterm-gui.exe",
    "alacritty.exe", "nvim.exe", "Godot_v4*.exe", "godot.exe", "Obsidian.exe",
]
titolo_contiene = [
    "docs.rs", "doc.rust-lang.org", "docs.godotengine.org", "stack overflow",
    "github.com", "claude code", "rust-lang",
]

[fuori_pista]
processi = ["Spotify.exe", "Discord.exe", "steam.exe", "steamwebhelper.exe", "vlc.exe"]
titolo_contiene = [
    "youtube", "reddit", "instagram", "twitch", "tiktok", "facebook",
    "netflix", "twitter", "x.com", "amazon.", "ebay", "9gag",
]
"####;

#[cfg(test)]
mod tests {
    use super::*;

    fn f(processo: &str, titolo: &str) -> Finestra {
        Finestra { processo: processo.into(), titolo: titolo.into() }
    }

    fn regole() -> Regole {
        Regole::default()
    }

    #[test]
    fn le_regole_di_default_sono_toml_valido_e_alla_versione_giusta() {
        let r = Regole::da_testo(REGOLE_DEFAULT).unwrap();
        assert_eq!(r.versione, VERSIONE_REGOLE);
        assert_eq!(r.soglie.suono_s, 10);
        assert!(!r.fuori_pista.titolo_contiene.is_empty());
    }

    #[test]
    fn una_versione_futura_si_rifiuta_invece_di_indovinare() {
        let errore = Regole::da_testo("versione = 99").unwrap_err();
        assert!(errore.contains("99"), "l'errore deve dire quale versione ha trovato");
    }

    /// I casi del §3 e del §4, uno per riga. Se questa tabella cambia, e'
    /// cambiata la cintura: deve cambiarla qualcuno di proposito.
    #[test]
    fn la_tabella_dei_verdetti() {
        let r = regole();
        let niente = Intento::default();
        let studio = Intento::nuovo("studio la concorrenza in Rust per l'esame", &[]);

        // L'editor e' in pista anche senza intento.
        assert_eq!(r.classifica(&f("Code.exe", "main.rs - rivals"), &niente), Verdetto::InPista);
        // Il titolo in pista basta su un processo sconosciuto (il browser).
        assert_eq!(
            r.classifica(&f("firefox.exe", "tokio - docs.rs — Mozilla Firefox"), &niente),
            Verdetto::InPista
        );
        // YouTube senza intento e' fuori.
        assert_eq!(
            r.classifica(&f("firefox.exe", "Lofi beats - YouTube"), &niente),
            Verdetto::FuoriPista
        );
        // ...e con l'intento giusto nel titolo, no. E' il §4.
        assert_eq!(
            r.classifica(&f("firefox.exe", "Rust concurrency explained - YouTube"), &studio),
            Verdetto::InPista
        );
        // Ma l'intento non sblocca un processo fuori pista.
        assert_eq!(
            r.classifica(&f("Spotify.exe", "Rust - Sleep Token"), &studio),
            Verdetto::FuoriPista
        );
        // Cio' che nessuna regola conosce resta incerto, e non richiama.
        assert_eq!(r.classifica(&f("explorer.exe", "Documenti"), &studio), Verdetto::Incerto);
        assert_eq!(r.classifica(&f("", ""), &studio), Verdetto::Incerto);
    }

    #[test]
    fn il_jolly_nei_processi_e_ancorato_ai_due_capi() {
        assert!(combacia("godot_v4*.exe", "godot_v4.3-stable.exe"));
        assert!(!combacia("godot_v4*.exe", "fakegodot_v4.exe"), "il prefisso e' ancorato");
        assert!(!combacia("godot_v4*.exe", "godot_v4.3-stable.exe.txt"), "il suffisso e' ancorato");
        assert!(combacia("code.exe", "code.exe"));
        assert!(!combacia("code.exe", "codex.exe"));
        assert!(combacia("*.exe", "qualunque.exe"));
    }

    #[test]
    fn dall_intento_escono_solo_le_parole_che_dicono_qualcosa() {
        let i = Intento::nuovo("oggi studio la concorrenza in Rust per l'esame", &[]);
        assert!(i.parole.contains(&"rust".to_string()));
        assert!(i.parole.contains(&"concorrenza".to_string()));
        assert!(i.parole.contains(&"esame".to_string()));
        assert!(!i.parole.contains(&"oggi".to_string()), "vuota");
        assert!(!i.parole.contains(&"studio".to_string()), "vuota");
        assert!(!i.parole.contains(&"per".to_string()), "troppo corta");
    }

    #[test]
    fn le_parole_date_a_mano_entrano_anche_corte() {
        let i = Intento::nuovo("sistemo il rating", &["elo".into(), "  ".into(), "WGPU".into()]);
        assert!(i.parole.contains(&"elo".to_string()));
        assert!(i.parole.contains(&"wgpu".to_string()), "minuscolo, per confrontare i titoli");
        assert!(!i.parole.contains(&String::new()), "gli spazi non sono una parola");
    }

    #[test]
    fn senza_intento_nessun_titolo_si_sblocca() {
        assert!(!Intento::default().sblocca("rust concurrency - youtube"));
    }
}
