//! suono.rs — l'uscita della cintura: un suono breve e le frasi registrate
//! (WO-01 §5 e §6).
//!
//! **Niente sintesi in tempo reale, per scelta del WO.** Le frasi sono 10-15 wav
//! generati una volta con Kokoro (`tools/genera_frasi.ps1`) e qui si suonano e
//! basta. La cintura deve poter partire in un secondo, non caricare un modello:
//! la voce che *parla* — quella dei rivali, generata sul momento — arriva dal
//! WO-04, e non ha niente a che vedere con questo file.
//!
//! `PlaySoundW` con `SND_ASYNC`: il ciclo della cintura batte una volta al
//! secondo e non deve mai fermarsi ad aspettare la fine di un wav. Un richiamo
//! che arrivasse tardi perche' il precedente stava ancora suonando falserebbe
//! l'unico numero che il WO chiede di misurare.
//!
//! Il suono breve del livello 1 non e' un file d'arte: lo genera `init` con
//! [`genera_richiamo`], due note e via. Un blob binario nel repo che nessuno
//! sa rigenerare e' un debito; dodici righe di seno no.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cintura::Livello;

/// Com'e' andata a finire. Il chiamante lo mette a registro: se le frasi non
/// sono state ancora generate, il registro deve dirlo — altrimenti in fondo
/// alla settimana si leggerebbero tre livelli di escalation che in realta' non
/// hanno mai fatto rumore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Esito {
    /// Il wav e' partito.
    Suonato { file: String },
    /// Il wav non c'era: e' stato stampato il testo della frase, col suono breve
    /// al posto della voce.
    Ripiego { testo: String },
    /// Niente da suonare e niente da dire: la cartella delle frasi e' vuota.
    Muto,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Frasi {
    #[serde(default)]
    pub frase: Vec<Frase>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Frase {
    /// 2 = frase registrata, 3 = frase piu' dura. Il livello 1 e' il suono
    /// breve e non ha testo.
    pub livello: u8,
    pub file: String,
    pub testo: String,
}

/// Le frasi di partenza. Sono in italiano, sono **neutre** e parlano della
/// finestra, mai di te: lo sfotto' e' dei rivali (WO-06), e i rivali passano
/// dal regista. Qui una frase che giudica sarebbe fuori principio.
pub const FRASI_DEFAULT: &str = r####"# frasi.toml — il testo dei richiami parlati (WO-01 §6)
#
# I wav si generano UNA VOLTA con Kokoro:  tools/genera_frasi.ps1
# Finche' non esistono, la cintura stampa il testo e suona il richiamo breve.
#
# Regola di scrittura, e non e' estetica: la cintura e' NEUTRA. Constata, non
# giudica; parla della finestra, mai della persona. Il rivale che punzecchia
# arriva col WO-06 e passa dal regista.

versione = 1

[[frase]]
livello = 2
file = "l2_01.wav"
testo = "Questa finestra non c'entra con quello che hai detto di fare."

[[frase]]
livello = 2
file = "l2_02.wav"
testo = "Sei fuori pista da quasi un minuto."

[[frase]]
livello = 2
file = "l2_03.wav"
testo = "Non e' la sessione che avevi dichiarato."

[[frase]]
livello = 2
file = "l2_04.wav"
testo = "Torna alla finestra di prima."

[[frase]]
livello = 2
file = "l2_05.wav"
testo = "Se serve, metti in pausa con un motivo."

[[frase]]
livello = 2
file = "l2_06.wav"
testo = "Questo non era nell'intento di stasera."

[[frase]]
livello = 3
file = "l3_01.wav"
testo = "Sono due minuti. O torni, o metti in pausa e dici perche'."

[[frase]]
livello = 3
file = "l3_02.wav"
testo = "La sessione e' ancora aperta e tu non ci sei."

[[frase]]
livello = 3
file = "l3_03.wav"
testo = "Due minuti fuori. Questa deviazione finisce a registro."

[[frase]]
livello = 3
file = "l3_04.wav"
testo = "Il tempo di questa sessione lo stai spendendo altrove."

[[frase]]
livello = 3
file = "l3_05.wav"
testo = "Decidi: torni al lavoro o chiudi la sessione."

[[frase]]
livello = 3
file = "l3_06.wav"
testo = "Ancora qui. Fermarsi si puo', ma si dice perche'."
"####;

/// Chi suona. Tiene il percorso dei file e abbastanza memoria da non ripetere
/// due volte di fila la stessa frase — che e' il modo piu' rapido di far
/// diventare un richiamo un rumore di fondo.
pub struct Voce {
    richiamo_breve: PathBuf,
    cartella_frasi: PathBuf,
    frasi: Frasi,
    ultima: Option<String>,
    seme: u64,
    /// Con `--muto` la cintura lavora e scrive il registro senza fare rumore:
    /// serve ai test di accettazione e alle prove in biblioteca.
    pub muto: bool,
}

impl Voce {
    pub fn nuova(richiamo_breve: PathBuf, cartella_frasi: PathBuf, frasi: Frasi, muto: bool) -> Self {
        let seme = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2026_0922)
            | 1;
        Self { richiamo_breve, cartella_frasi, frasi, ultima: None, seme, muto }
    }

    /// Suona il richiamo del livello dato e dice com'e' andata.
    pub fn richiama(&mut self, livello: Livello) -> Esito {
        if livello == Livello::Suono {
            if !self.muto {
                riproduci(&self.richiamo_breve);
            }
            return Esito::Suonato { file: nome_file(&self.richiamo_breve) };
        }

        let Some(frase) = self.scegli(livello.numero()) else {
            return Esito::Muto;
        };
        let percorso = self.cartella_frasi.join(&frase.file);
        if percorso.is_file() {
            if !self.muto {
                riproduci(&percorso);
            }
            Esito::Suonato { file: frase.file }
        } else {
            // Il wav non c'e' ancora (non ancora generato): il suono
            // breve tiene il posto della voce e il testo va sul terminale.
            if !self.muto {
                riproduci(&self.richiamo_breve);
            }
            Esito::Ripiego { testo: frase.testo }
        }
    }

    /// Una frase del livello, mai due volte di fila la stessa.
    fn scegli(&mut self, livello: u8) -> Option<Frase> {
        // Le candidate si clonano invece di prenderle in prestito: `frasi` e'
        // una dozzina di struct minuscole, e in cambio `self` resta libero per
        // `prossimo_caso`, che lo vuole mutabile.
        let candidate: Vec<Frase> =
            self.frasi.frase.iter().filter(|f| f.livello == livello).cloned().collect();
        if candidate.is_empty() {
            return None;
        }
        let diverse: Vec<Frase> = candidate
            .iter()
            .filter(|f| Some(&f.file) != self.ultima.as_ref())
            .cloned()
            .collect();
        let pool = if diverse.is_empty() { candidate } else { diverse };
        let quale = (self.prossimo_caso() % pool.len() as u64) as usize;
        let scelta = pool[quale].clone();
        self.ultima = Some(scelta.file.clone());
        Some(scelta)
    }

    /// xorshift64*: dodici righe invece del crate `rand`, e nessuno qui deve
    /// scegliere una chiave crittografica — solo una frase su sei.
    fn prossimo_caso(&mut self) -> u64 {
        let mut x = self.seme;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.seme = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33
    }
}

fn nome_file(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn riproduci(path: &Path) {
    use std::os::raw::c_void;
    use std::os::windows::ffi::OsStrExt;

    const SND_ASYNC: u32 = 0x0001;
    const SND_NODEFAULT: u32 = 0x0002;
    const SND_FILENAME: u32 = 0x0002_0000;

    #[link(name = "winmm")]
    unsafe extern "system" {
        fn PlaySoundW(nome: *const u16, modulo: *mut c_void, flag: u32) -> i32;
    }

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    // SAFETY: `wide` e' NUL-terminata e vive per tutta la chiamata. SND_ASYNC fa
    // tornare subito: e' quello che vogliamo, il ciclo non deve aspettare.
    // SND_NODEFAULT evita il "din" di sistema se il file manca o e' rotto.
    unsafe {
        PlaySoundW(wide.as_ptr(), std::ptr::null_mut(), SND_FILENAME | SND_ASYNC | SND_NODEFAULT);
    }
}

#[cfg(not(target_os = "windows"))]
fn riproduci(_path: &Path) {}

// ── Il wav del richiamo breve ────────────────────────────────────────────────

const FREQUENZA: u32 = 22_050;

/// Genera il suono del livello 1: due note corte, discendenti.
///
/// Discendenti e non ascendenti di proposito: un intervallo che sale suona come
/// una notifica che annuncia qualcosa, uno che scende suona come un "ehi".
/// Dura ~260 ms — abbastanza da sentirsi in cuffia sopra la musica, abbastanza
/// poco da non coprire una parola se stai parlando.
pub fn genera_richiamo(path: &Path) -> std::io::Result<()> {
    let mut campioni: Vec<i16> = Vec::new();
    campioni.extend(nota(880.0, 0.10, 0.35));
    campioni.extend(silenzio(0.03));
    campioni.extend(nota(587.33, 0.13, 0.35));
    scrivi_wav(path, &campioni)
}

fn nota(hz: f32, secondi: f32, ampiezza: f32) -> Vec<i16> {
    let n = (FREQUENZA as f32 * secondi) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / FREQUENZA as f32;
            // Inviluppo a coseno rialzato: senza, l'attacco e lo stacco netti
            // fanno un "clic" che si sente piu' della nota.
            let inviluppo = {
                let rampa = (FREQUENZA as f32 * 0.008) as usize;
                let su = (i as f32 / rampa as f32).min(1.0);
                let giu = ((n - i) as f32 / rampa as f32).min(1.0);
                su.min(giu)
            };
            let v = (t * hz * std::f32::consts::TAU).sin() * ampiezza * inviluppo;
            (v * i16::MAX as f32) as i16
        })
        .collect()
}

fn silenzio(secondi: f32) -> Vec<i16> {
    vec![0; (FREQUENZA as f32 * secondi) as usize]
}

/// WAV PCM 16 bit mono. Quarantaquattro byte di intestazione scritti a mano:
/// meno di quanto costerebbe spiegare perche' c'e' una dipendenza in piu'.
pub fn scrivi_wav(path: &Path, campioni: &[i16]) -> std::io::Result<()> {
    use std::io::Write;

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let dati = campioni.len() as u32 * 2;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);

    f.write_all(b"RIFF")?;
    f.write_all(&(36 + dati).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?; // dimensione del blocco fmt
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&FREQUENZA.to_le_bytes())?;
    f.write_all(&(FREQUENZA * 2).to_le_bytes())?; // byte al secondo
    f.write_all(&2u16.to_le_bytes())?; // allineamento di blocco
    f.write_all(&16u16.to_le_bytes())?; // bit per campione
    f.write_all(b"data")?;
    f.write_all(&dati.to_le_bytes())?;
    for c in campioni {
        f.write_all(&c.to_le_bytes())?;
    }
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frasi() -> Frasi {
        toml::from_str(FRASI_DEFAULT).unwrap()
    }

    #[test]
    fn le_frasi_di_default_coprono_i_due_livelli_parlati() {
        let f = frasi();
        let l2 = f.frase.iter().filter(|x| x.livello == 2).count();
        let l3 = f.frase.iter().filter(|x| x.livello == 3).count();
        assert!(l2 >= 5 && l3 >= 5, "l2={l2} l3={l3}");
        // §6: da 10 a 15 file.
        assert!((10..=15).contains(&f.frase.len()), "sono {}", f.frase.len());
        assert!(f.frase.iter().all(|x| x.file.ends_with(".wav") && !x.testo.is_empty()));
    }

    #[test]
    fn non_si_ripete_la_stessa_frase_due_volte_di_fila() {
        let mut v = Voce::nuova(PathBuf::from("x.wav"), PathBuf::from("frasi"), frasi(), true);
        let mut precedente = String::new();
        for _ in 0..40 {
            let scelta = v.scegli(3).expect("ci sono frasi di livello 3");
            assert_ne!(scelta.file, precedente, "due volte di fila e diventa rumore di fondo");
            precedente = scelta.file;
        }
    }

    #[test]
    fn senza_il_wav_la_frase_finisce_sul_terminale_invece_di_sparire() {
        let mut v = Voce::nuova(
            PathBuf::from("richiamo.wav"),
            PathBuf::from("cartella-che-non-esiste"),
            frasi(),
            true,
        );
        match v.richiama(Livello::Frase) {
            Esito::Ripiego { testo } => assert!(!testo.is_empty()),
            altro => panic!("il richiamo non puo' evaporare in silenzio: {altro:?}"),
        }
    }

    #[test]
    fn senza_nemmeno_una_frase_la_cintura_lo_dichiara() {
        let mut v = Voce::nuova(PathBuf::from("r.wav"), PathBuf::from("."), Frasi { frase: vec![] }, true);
        assert_eq!(v.richiama(Livello::Dura), Esito::Muto);
        // Il livello 1 pero' suona sempre: non dipende dalle frasi.
        assert!(matches!(v.richiama(Livello::Suono), Esito::Suonato { .. }));
    }

    #[test]
    fn il_wav_generato_ha_un_intestazione_valida_e_dura_quanto_promesso() {
        let dir = std::env::temp_dir().join("rivals-belt-test-wav");
        let path = dir.join("richiamo.wav");
        genera_richiamo(&path).unwrap();
        let b = std::fs::read(&path).unwrap();

        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize, b.len() - 8);
        let dati = u32::from_le_bytes(b[40..44].try_into().unwrap()) as usize;
        assert_eq!(dati, b.len() - 44);

        let durata_s = dati as f32 / 2.0 / FREQUENZA as f32;
        assert!((0.25..0.28).contains(&durata_s), "dura {durata_s} s");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
