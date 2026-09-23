//! registro.rs — **N4, il registro di sessione** (WO-01 §8).
//!
//! Una riga JSON per evento, in append, un file per giornata:
//! `registro/2026-09-22.jsonl`. Il formato e' quello perche' deve reggere tre
//! usi diversi senza cambiare: lo legge il report (`rivals-belt report`), lo
//! legge la verifica dell'accettazione (`rivals-belt verifica`), e lo leggerai
//! **tu**, a occhio, a fine settimana. Un binario che nessuno sa aprire non e'
//! un registro, e' una scatola nera.
//!
//! # Tre scelte che non sono dettagli
//!
//! - **Si scrive in chiaro.** Il WO-03 chiedera' la stessa cosa per il registro
//!   di uscita verso l'API ("ogni payload salvato in chiaro in locale"): e' la
//!   prova di fiducia di un'app che guarda lo schermo. Si comincia da qui.
//! - **`flush` a ogni riga.** Un buffer che si perde nel crash porta via
//!   esattamente i minuti che interessano. Costa un `write` al secondo nel caso
//!   peggiore: niente, in confronto.
//! - **Ogni riga porta due tempi.** `ts` e' l'ora locale, per leggere e per
//!   sapere qual e' la tua fascia oraria peggiore; `mono_ms` e' l'orologio
//!   monotono della sessione, ed e' l'unico con cui si misurano le durate.
//!   → [`crate::tempo`]
//!
//! Il campo `v` e' la versione dello schema (formati con versione e migrazione). Le righe di una versione che non conosciamo si **saltano**
//! invece di far fallire il report: un registro vecchio deve restare leggibile
//! per la parte che si capisce.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tempo::{ora_locale, Orologio};

pub const VERSIONE_REGISTRO: u32 = 1;

/// Una riga del registro. Tutti i campi oltre i primi cinque sono opzionali e
/// spariscono dal JSON se vuoti: una riga di richiamo non deve portarsi dietro
/// dieci `null`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Riga {
    pub v: u32,
    /// Ora locale leggibile: `2026-09-22T21:04:07.123`.
    pub ts: String,
    /// Millisecondi dall'avvio della sessione (orologio monotono).
    pub mono_ms: u64,
    /// Id della sessione: le sessioni di una giornata stanno nello stesso file.
    pub sessione: String,
    pub tipo: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub processo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titolo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdetto: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub livello: Option<u8>,
    /// Il numero dell'accettazione n. 1: millisecondi fra la deviazione vista e
    /// il richiamo partito.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ritardo_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ripetizione: Option<bool>,
    /// La soglia che ha fatto scattare *questo* richiamo, in millisecondi. Va
    /// scritta sulla riga e non dedotta dal `rules.toml` di oggi: le soglie si
    /// cambiano, e un registro di tre settimane fa deve restare verificabile
    /// con i numeri che aveva allora.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soglia_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durata_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ritorno_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chiusura: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub esito: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intento: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parole: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nota: Option<String>,
}

impl Riga {
    /// L'ora del giorno, `0..=23`, ricavata da `ts`. E' il numero con cui il
    /// report trova la fascia oraria peggiore.
    pub fn ora_del_giorno(&self) -> Option<u32> {
        let t = self.ts.split('T').nth(1)?;
        t.split(':').next()?.parse().ok()
    }
}

pub struct Registro {
    file: std::fs::File,
    orologio: Orologio,
    sessione: String,
    percorso: PathBuf,
}

impl Registro {
    /// Apre (o crea) il file della giornata e annota l'apertura della sessione.
    ///
    /// L'id di sessione e' l'ora d'inizio: e' leggibile, ordina da solo, e due
    /// sessioni nello stesso secondo non esistono.
    pub fn apri(cartella: &Path, orologio: Orologio) -> std::io::Result<Self> {
        std::fs::create_dir_all(cartella)?;
        let ora = ora_locale();
        let percorso = cartella.join(format!("{}.jsonl", ora.giorno_iso()));
        let file = std::fs::OpenOptions::new().create(true).append(true).open(&percorso)?;
        let sessione = format!("{:02}{:02}{:02}", ora.ora, ora.minuto, ora.secondo);
        Ok(Self { file, orologio, sessione, percorso })
    }

    pub fn percorso(&self) -> &Path {
        &self.percorso
    }

    pub fn sessione(&self) -> &str {
        &self.sessione
    }

    /// Scrive una riga e la porta subito su disco.
    pub fn scrivi(&mut self, mut riga: Riga) {
        riga.v = VERSIONE_REGISTRO;
        riga.ts = ora_locale().iso();
        riga.mono_ms = self.orologio.ms();
        riga.sessione = self.sessione.clone();
        match serde_json::to_string(&riga) {
            Ok(s) => {
                // Un errore di scrittura non deve fermare la cintura: meglio una
                // cintura che richiama senza registro che nessuna cintura. Ma si
                // dice, forte, perche' senza registro l'accettazione non esiste.
                if let Err(e) = writeln!(self.file, "{s}").and_then(|_| self.file.flush()) {
                    eprintln!("[registro] riga persa: {e}");
                }
            }
            Err(e) => eprintln!("[registro] riga non serializzabile: {e}"),
        }
    }

    /// Scorciatoia per gli eventi senza campi.
    pub fn evento(&mut self, tipo: &str) {
        self.scrivi(Riga { tipo: tipo.to_string(), ..Default::default() });
    }
}

/// Legge un file di registro. Le righe illeggibili o di una versione futura si
/// saltano in silenzio: un report che muore per una riga storta e' un report
/// che non si usa.
pub fn leggi(percorso: &Path) -> std::io::Result<Vec<Riga>> {
    let testo = std::fs::read_to_string(percorso)?;
    Ok(righe_da_testo(&testo))
}

pub fn righe_da_testo(testo: &str) -> Vec<Riga> {
    testo
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Riga>(l).ok())
        .filter(|r| r.v == VERSIONE_REGISTRO)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_riga_di_richiamo_non_si_porta_dietro_i_campi_vuoti() {
        let riga = Riga {
            v: 1,
            ts: "2026-09-22T21:04:07.123".into(),
            mono_ms: 10_000,
            sessione: "210357".into(),
            tipo: "richiamo".into(),
            livello: Some(1),
            ritardo_ms: Some(10_400),
            ..Default::default()
        };
        let json = serde_json::to_string(&riga).unwrap();
        assert!(!json.contains("null"), "niente null a riempire il file: {json}");
        assert!(json.contains(r#""tipo":"richiamo""#));
        assert!(json.contains(r#""ritardo_ms":10400"#));
    }

    #[test]
    fn le_righe_di_una_versione_che_non_conosciamo_si_saltano() {
        let testo = "\
{\"v\":1,\"ts\":\"2026-09-22T09:00:00.000\",\"mono_ms\":0,\"sessione\":\"a\",\"tipo\":\"sessione_aperta\"}
{\"v\":99,\"ts\":\"2026-09-22T09:00:01.000\",\"mono_ms\":1,\"sessione\":\"a\",\"tipo\":\"dal futuro\"}
questa riga non e' nemmeno json

{\"v\":1,\"ts\":\"2026-09-22T09:00:02.000\",\"mono_ms\":2,\"sessione\":\"a\",\"tipo\":\"richiamo\",\"livello\":1}";
        let righe = righe_da_testo(testo);
        assert_eq!(righe.len(), 2, "una salta per versione, una perche' non e' json");
        assert_eq!(righe[1].livello, Some(1));
    }

    #[test]
    fn dall_ora_locale_esce_la_fascia_oraria() {
        let mut r = Riga::default();
        r.ts = "2026-09-22T23:59:59.999".into();
        assert_eq!(r.ora_del_giorno(), Some(23));
        r.ts = "2026-09-22T07:05:00.000".into();
        assert_eq!(r.ora_del_giorno(), Some(7));
        r.ts = "niente".into();
        assert_eq!(r.ora_del_giorno(), None);
    }

    #[test]
    fn il_registro_scrive_davvero_e_si_rilegge() {
        let dir = std::env::temp_dir().join("rivals-belt-test-registro");
        let _ = std::fs::remove_dir_all(&dir);
        let mut reg = Registro::apri(&dir, Orologio::nuovo()).unwrap();
        reg.evento("sessione_aperta");
        reg.scrivi(Riga { tipo: "richiamo".into(), livello: Some(2), ritardo_ms: Some(45_300), ..Default::default() });

        let righe = leggi(reg.percorso()).unwrap();
        assert_eq!(righe.len(), 2);
        assert_eq!(righe[0].tipo, "sessione_aperta");
        assert_eq!(righe[1].ritardo_ms, Some(45_300));
        assert_eq!(righe[0].sessione, righe[1].sessione, "stessa sessione, stesso id");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
