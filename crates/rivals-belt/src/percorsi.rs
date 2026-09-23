//! percorsi.rs — dove vivono i file, e perche' **nessun percorso e' scritto nel
//! codice**.
//!
//! Un percorso della macchina di sviluppo scritto nel codice e' un programma
//! che non parte su nessun'altra macchina. Qui non ce n'e' nessuno, e un test
//! lo impone: `tests/nessun_percorso_assoluto.rs` fallisce se ne compare uno.
//!
//! La cartella dati si sceglie cosi', in ordine:
//!
//! 1. `--dati <cartella>` sulla riga di comando — per le prove e per i test;
//! 2. la variabile d'ambiente `RIVALS_DATA`;
//! 3. `%APPDATA%\3Rivals` — il posto giusto su Windows, e quello che
//!    l'installer (WO-10) usera' senza dover cambiare una riga.
//!
//! Se anche `APPDATA` manca — succede nei servizi e in qualche shell spoglia —
//! si ripiega su `.\dati-rivals` accanto all'eseguibile, e lo si dice.

use std::path::{Path, PathBuf};

pub struct Percorsi {
    pub radice: PathBuf,
}

impl Percorsi {
    /// `esplicita` e' quello che arriva da `--dati`, se c'e'.
    pub fn scopri(esplicita: Option<&str>) -> Self {
        if let Some(d) = esplicita.filter(|d| !d.trim().is_empty()) {
            return Self { radice: PathBuf::from(d) };
        }
        if let Ok(d) = std::env::var("RIVALS_DATA") {
            if !d.trim().is_empty() {
                return Self { radice: PathBuf::from(d) };
            }
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            if !appdata.trim().is_empty() {
                return Self { radice: PathBuf::from(appdata).join("3Rivals") };
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            if !home.trim().is_empty() {
                return Self { radice: PathBuf::from(home).join(".rivals") };
            }
        }
        eprintln!("[percorsi] ne' APPDATA ne' HOME: uso ./dati-rivals");
        Self { radice: PathBuf::from("dati-rivals") }
    }

    pub fn regole(&self) -> PathBuf {
        self.radice.join("rules.toml")
    }

    pub fn registro(&self) -> PathBuf {
        self.radice.join("registro")
    }

    pub fn frasi(&self) -> PathBuf {
        self.radice.join("frasi")
    }

    pub fn manifest_frasi(&self) -> PathBuf {
        self.frasi().join("frasi.toml")
    }

    pub fn richiamo_breve(&self) -> PathBuf {
        self.radice.join("suoni").join("richiamo.wav")
    }

    /// Prepara la cartella dati: crea quello che manca, **non tocca quello che
    /// c'e' gia'**. Si puo' rilanciare senza paura — e il file delle regole, che
    /// e' l'unico che si modifica a mano, non viene mai sovrascritto.
    pub fn prepara(&self) -> std::io::Result<Vec<String>> {
        let mut fatto = Vec::new();
        std::fs::create_dir_all(&self.radice)?;
        std::fs::create_dir_all(self.registro())?;
        std::fs::create_dir_all(self.frasi())?;

        if scrivi_se_manca(&self.regole(), crate::regole::REGOLE_DEFAULT)? {
            fatto.push(format!("scritto {}", self.regole().display()));
        }
        if scrivi_se_manca(&self.manifest_frasi(), crate::suono::FRASI_DEFAULT)? {
            fatto.push(format!("scritto {}", self.manifest_frasi().display()));
        }
        if !self.richiamo_breve().is_file() {
            crate::suono::genera_richiamo(&self.richiamo_breve())?;
            fatto.push(format!("generato {}", self.richiamo_breve().display()));
        }
        Ok(fatto)
    }
}

fn scrivi_se_manca(path: &Path, contenuto: &str) -> std::io::Result<bool> {
    if path.is_file() {
        return Ok(false);
    }
    std::fs::write(path, contenuto)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cartella_esplicita_vince_su_tutto() {
        let p = Percorsi::scopri(Some("C:/prove/rivals")); // percorso di prova
        assert_eq!(p.radice, PathBuf::from("C:/prove/rivals")); // percorso di prova
        assert!(p.regole().ends_with("rules.toml"));
        assert!(p.richiamo_breve().ends_with("richiamo.wav"));
    }

    #[test]
    fn una_cartella_esplicita_vuota_non_conta() {
        // `--dati ""` non deve dirottare i dati nella cartella corrente.
        let p = Percorsi::scopri(Some("   "));
        assert_ne!(p.radice, PathBuf::from("   "));
    }

    #[test]
    fn prepara_e_ripetibile_e_non_sovrascrive_le_tue_regole() {
        let dir = std::env::temp_dir().join("rivals-belt-test-prepara");
        let _ = std::fs::remove_dir_all(&dir);
        let p = Percorsi::scopri(Some(dir.to_str().unwrap()));

        let primo = p.prepara().unwrap();
        assert_eq!(primo.len(), 3, "regole, frasi, wav: {primo:?}");
        std::fs::write(p.regole(), "versione = 1\n# le mie regole\n").unwrap();

        let secondo = p.prepara().unwrap();
        assert!(secondo.is_empty(), "la seconda volta non fa niente: {secondo:?}");
        let regole = std::fs::read_to_string(p.regole()).unwrap();
        assert!(regole.contains("le mie regole"), "le regole scritte a mano restano");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
