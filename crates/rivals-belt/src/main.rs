//! rivals-belt — **la cintura** (WO-01), il primo pezzo di 3Rivals.
//!
//! Guarda la finestra attiva una volta al secondo, la confronta con quello che
//! hai dichiarato di fare, e se esci di pista ti richiama: un suono dopo dieci
//! secondi, una frase dopo quarantacinque, una frase piu' dura dopo due minuti.
//! Si ferma appena torni.
//!
//! **Non c'e' nessuna AI qui dentro, ed e' voluto.** La cintura e' un circuito
//! locale, neutro e chiuso: vede solo il titolo della finestra, non manda niente
//! da nessuna parte, e non giudica. I rivali che ragionano — API, voce,
//! interrogazioni — arrivano dalla Fase 2 in poi e passeranno dal regista.
//! Se un giorno questo file importasse un client HTTP, il fork avrebbe perso il
//! suo pezzo piu' utile.
//!
//! ```text
//! rivals-belt                      apre una sessione e chiede l'intento
//! rivals-belt --intento "..."      apre una sessione senza domande
//! rivals-belt init                 prepara la cartella dati e i file
//! rivals-belt report [2026-09-22]  il report della giornata
//! rivals-belt verifica [giorno]    i criteri di accettazione, sui numeri veri
//! ```

mod cintura;
mod finestra;
mod percorsi;
mod registro;
mod regole;
mod report;
mod suono;
mod tempo;

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use cintura::{Azione, Chiusura, Cintura, Livello};
use percorsi::Percorsi;
use registro::{Registro, Riga};
use regole::{Intento, Regole, Soglie, Verdetto};
use suono::{Esito, Frasi, Voce};
use tempo::{durata_umana, ora_locale, Orologio};

/// Alzata dal gestore di Ctrl+C. Il ciclo la guarda a ogni giro e chiude la
/// sessione come si deve: senza, l'ultima deviazione resterebbe aperta e il
/// report di quella serata sarebbe sbagliato proprio sul pezzo che interessa.
static USCITA: AtomicBool = AtomicBool::new(false);

fn main() -> ExitCode {
    let argomenti: Vec<String> = std::env::args().skip(1).collect();
    let dati = valore_opzione(&argomenti, "--dati");
    let percorsi = Percorsi::scopri(dati.as_deref());

    let comando = argomenti.first().map(|s| s.as_str()).unwrap_or("sessione");
    let esito = match comando {
        "init" => comando_init(&percorsi),
        "report" => comando_report(&percorsi, argomenti.get(1).filter(|a| !a.starts_with("--"))),
        "verifica" => comando_verifica(&percorsi, argomenti.get(1).filter(|a| !a.starts_with("--"))),
        "aiuto" | "--help" | "-h" => {
            stampa_aiuto();
            Ok(true)
        }
        _ => comando_sessione(&percorsi, &argomenti),
    };

    match esito {
        Ok(true) => ExitCode::SUCCESS,
        // Un criterio di accettazione non verde non e' un errore del programma:
        // e' un esito. Esce con 1 perche' uno script possa accorgersene.
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("\n[errore] {e}");
            ExitCode::from(2)
        }
    }
}

fn stampa_aiuto() {
    println!(
        "rivals-belt — la cintura di 3Rivals (WO-01)\n\
         \n\
         USO\n\
         \x20 rivals-belt [sessione] [--intento \"cosa fai\"] [--parole a,b] [--muto]\n\
         \x20 rivals-belt init                    prepara la cartella dati\n\
         \x20 rivals-belt report [2026-09-22]     il report della giornata\n\
         \x20 rivals-belt verifica [2026-09-22]   i criteri di accettazione del WO-01\n\
         \n\
         OVUNQUE\n\
         \x20 --dati <cartella>   dove stanno regole, registro e frasi\n\
         \x20                     (di default %APPDATA%\\3Rivals, o $RIVALS_DATA)\n\
         \n\
         DURANTE UNA SESSIONE, si scrive nel terminale\n\
         \x20 pausa <minuti> <motivo>   sospende la cintura; il motivo va a registro\n\
         \x20 riprendi                  torna a guardare prima che la pausa scada\n\
         \x20 stato                     dove sei e da quanto\n\
         \x20 esci                      chiude la sessione e chiede il tuo verdetto"
    );
}

fn comando_init(p: &Percorsi) -> Result<bool, String> {
    let fatto = p.prepara().map_err(|e| format!("non riesco a preparare {}: {e}", p.radice.display()))?;
    println!("Cartella dati: {}", p.radice.display());
    if fatto.is_empty() {
        println!("C'era gia' tutto. Niente e' stato toccato.");
    } else {
        for f in fatto {
            println!("  {f}");
        }
    }
    println!(
        "\nLe frasi parlate sono ancora da generare: apri {} e lancia tools/genera_frasi.ps1.\n\
         Finche' mancano, la cintura suona il richiamo breve e stampa il testo.",
        p.frasi().display()
    );
    Ok(true)
}

fn comando_report(p: &Percorsi, giorno: Option<&String>) -> Result<bool, String> {
    let giorno = giorno.cloned().unwrap_or_else(|| ora_locale().giorno_iso());
    let file = p.registro().join(format!("{giorno}.jsonl"));
    let righe = registro::leggi(&file).unwrap_or_default();
    print!("{}", report::testo(&giorno, &righe));
    if righe.is_empty() {
        println!("(cercato in {})", file.display());
    }
    Ok(true)
}

fn comando_verifica(p: &Percorsi, giorno: Option<&String>) -> Result<bool, String> {
    let giorno = giorno.cloned().unwrap_or_else(|| ora_locale().giorno_iso());
    let file = p.registro().join(format!("{giorno}.jsonl"));
    let righe = registro::leggi(&file).unwrap_or_default();
    let (testo, ok) = report::verifica(&giorno, &righe);
    print!("{testo}");
    Ok(ok)
}

// ── La sessione ──────────────────────────────────────────────────────────────

fn comando_sessione(p: &Percorsi, argomenti: &[String]) -> Result<bool, String> {
    p.prepara().map_err(|e| format!("non riesco a preparare {}: {e}", p.radice.display()))?;

    let regole = if p.regole().is_file() { Regole::da_file(&p.regole())? } else { Regole::default() };
    let soglie = regole.soglie;
    let frasi: Frasi = std::fs::read_to_string(p.manifest_frasi())
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_else(|| toml::from_str(suono::FRASI_DEFAULT).expect("le frasi di default sono valide"));

    let (comandi_tx, comandi) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        // Il terminale su un thread suo: il ciclo della cintura non deve mai
        // fermarsi ad aspettare che tu scriva qualcosa.
        let stdin = std::io::stdin();
        let mut riga = String::new();
        loop {
            riga.clear();
            match stdin.read_line(&mut riga) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if comandi_tx.send(riga.trim().to_string()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // §4 — l'intento di sessione. Senza, la cintura non sa cosa sbloccare, e
    // qualunque titolo ambiguo resta ambiguo.
    let testo_intento = match valore_opzione(argomenti, "--intento") {
        Some(t) => t,
        None => {
            println!("Cosa fai in questa sessione? (una riga, invio per confermare)");
            print!("> ");
            usa_stdout();
            comandi.recv().map_err(|_| "terminale chiuso prima dell'intento".to_string())?
        }
    };
    let extra: Vec<String> = valore_opzione(argomenti, "--parole")
        .map(|s| s.split(',').map(|w| w.trim().to_string()).collect())
        .unwrap_or_default();
    let intento = Intento::nuovo(&testo_intento, &extra);
    let muto = argomenti.iter().any(|a| a == "--muto");

    let orologio = Orologio::nuovo();
    let mut reg = Registro::apri(&p.registro(), orologio)
        .map_err(|e| format!("non riesco ad aprire il registro: {e}"))?;
    let mut voce = Voce::nuova(p.richiamo_breve(), p.frasi(), frasi, muto);
    let mut belt = Cintura::nuova(soglie);

    reg.scrivi(Riga {
        tipo: "sessione_aperta".into(),
        intento: Some(intento.testo.clone()),
        parole: Some(intento.parole.clone()),
        ..Default::default()
    });

    registra_ctrl_c();
    println!(
        "\nCintura attiva — sessione {} · {}\n\
         intento: {}\n\
         parole che sbloccano i titoli ambigui: {}\n\
         soglie: suono {} s · frase {} s · frase dura {} s, poi ogni {} s\n\
         registro: {}\n\
         {}\n\
         Scrivi `aiuto` per i comandi, `esci` per chiudere.\n",
        reg.sessione(),
        ora_locale().iso(),
        if intento.testo.is_empty() { "(nessuno)" } else { &intento.testo },
        if intento.parole.is_empty() { "(nessuna)".to_string() } else { intento.parole.join(", ") },
        soglie.suono_s,
        soglie.frase_s,
        soglie.frase_dura_s,
        soglie.ripetizione_s,
        reg.percorso().display(),
        if muto { "MUTO: nessun suono, solo registro." } else { "" }
    );

    let mut ultima_finestra = finestra::Finestra::default();
    let mut ultimo_verdetto = Verdetto::Incerto;
    let mut ultima_riga_ms = 0u64;
    let mut uscita_pulita = true;

    loop {
        if USCITA.load(Ordering::Relaxed) {
            uscita_pulita = false;
            break;
        }

        // 1 · i comandi arrivati dal terminale
        let mut esci = false;
        while let Ok(riga) = comandi.try_recv() {
            match esegui_comando(&riga, &mut belt, &mut reg, orologio, &intento) {
                Comando::Esci => esci = true,
                Comando::Niente => {}
            }
        }
        if esci {
            break;
        }

        // 2 · il sensore
        let ora_ms = orologio.ms();
        let vista = finestra::finestra_attiva().unwrap_or_default();
        let verdetto = regole.classifica(&vista, &intento);

        if merita_una_riga(&vista, &ultima_finestra, verdetto, ultimo_verdetto, ora_ms, ultima_riga_ms) {
            reg.scrivi(Riga {
                tipo: "finestra".into(),
                processo: Some(vista.processo.clone()),
                titolo: Some(vista.titolo.clone()),
                verdetto: Some(verdetto.come_stringa().to_string()),
                ..Default::default()
            });
            ultima_riga_ms = ora_ms;
        }
        ultima_finestra = vista.clone();
        ultimo_verdetto = verdetto;

        // 3 · la cintura
        for azione in belt.osserva(ora_ms, &vista, verdetto) {
            esegui_azione(azione, &mut voce, &mut reg, soglie);
        }

        std::thread::sleep(Duration::from_millis(soglie.intervallo_ms));
    }

    // Chiusura: la deviazione aperta si chiude, poi il verdetto e' tuo.
    if let Some(fine) = belt.chiudi_sessione(orologio.ms()) {
        esegui_azione(fine, &mut voce, &mut reg, soglie);
    }
    if uscita_pulita {
        chiedi_verdetto(&comandi, &mut reg);
    } else {
        println!("\nInterrotto. La sessione e' chiusa nel registro, il verdetto no.");
    }
    reg.scrivi(Riga {
        tipo: "sessione_chiusa".into(),
        durata_ms: Some(orologio.ms()),
        ..Default::default()
    });

    let righe = registro::leggi(reg.percorso()).unwrap_or_default();
    let solo_questa: Vec<Riga> = righe.into_iter().filter(|r| r.sessione == reg.sessione()).collect();
    println!("\n{}", report::testo(&ora_locale().giorno_iso(), &solo_questa));
    Ok(true)
}

/// Ogni quanto, al massimo, si riscrive una riga `finestra` per lo stesso
/// processo che ha solo cambiato titolo.
const RESPIRO_RIGHE_MS: u64 = 5_000;

/// Questa finestra merita una riga di registro?
///
/// Un cambio di finestra o di verdetto si scrive sempre: e' il segnale. Un
/// titolo che cambia da solo, no. Misurato il 22/09 alla prima prova vera: il
/// titolo di Windows Terminal porta uno spinner (`◑ ◐`) che gira una volta al
/// secondo, e il registro prendeva **una riga al secondo** per una finestra
/// sola. Stesso difetto con i player (il tempo che scorre nel titolo) e con
/// gli editor che mostrano la posizione del cursore.
///
/// Non e' solo peso: il censimento delle finestre incerte — il campione
/// etichettato che serve al classificatore del WO-05 — verrebbe dominato da
/// venti fotogrammi della stessa finestra.
fn merita_una_riga(
    vista: &finestra::Finestra,
    ultima: &finestra::Finestra,
    verdetto: Verdetto,
    ultimo_verdetto: Verdetto,
    ora_ms: u64,
    ultima_riga_ms: u64,
) -> bool {
    if vista.processo != ultima.processo || verdetto != ultimo_verdetto {
        return true;
    }
    if vista.titolo == ultima.titolo {
        return false;
    }
    ora_ms.saturating_sub(ultima_riga_ms) >= RESPIRO_RIGHE_MS
}

enum Comando {
    Niente,
    Esci,
}

fn esegui_comando(
    riga: &str,
    belt: &mut Cintura,
    reg: &mut Registro,
    orologio: Orologio,
    intento: &Intento,
) -> Comando {
    let riga = riga.trim();
    if riga.is_empty() {
        return Comando::Niente;
    }
    let mut pezzi = riga.split_whitespace();
    match pezzi.next().unwrap_or("").to_lowercase().as_str() {
        "esci" | "fine" | "stop" => return Comando::Esci,
        "pausa" => {
            // §7 — `pausa <minuti> <motivo>`. Il motivo non e' opzionale: e' il
            // punto del comando. "Fermarsi si puo', ma costa: dici perche' e
            // resta a registro."
            let minuti: u64 = pezzi.next().and_then(|m| m.parse().ok()).unwrap_or(0);
            let motivo: String = pezzi.collect::<Vec<_>>().join(" ");
            if minuti == 0 || motivo.trim().is_empty() {
                println!("  uso: pausa <minuti> <motivo>   — il motivo serve, e resta scritto.");
                return Comando::Niente;
            }
            let durata = minuti * 60_000;
            for azione in belt.pausa(orologio.ms(), durata, &motivo) {
                if let Azione::Fine { durata_ms, livello_max, chiusura, .. } = azione {
                    scrivi_fine(reg, durata_ms, None, livello_max, chiusura);
                }
            }
            reg.scrivi(Riga {
                tipo: "pausa".into(),
                durata_ms: Some(durata),
                motivo: Some(motivo.clone()),
                ..Default::default()
            });
            println!("  in pausa per {minuti} minuti — {motivo}");
        }
        "riprendi" => {
            if belt.in_pausa() {
                belt.riprendi();
                reg.evento("ripresa");
                println!("  cintura di nuovo attiva.");
            } else {
                println!("  non eri in pausa.");
            }
        }
        "stato" => {
            if let Some(motivo) = belt.motivo_pausa() {
                println!("  in pausa — {motivo}");
            } else if let Some(ms) = belt.fuori_da(orologio.ms()) {
                println!("  fuori pista da {}", durata_umana(ms));
            } else {
                println!("  in pista. Intento: {}", intento.testo);
            }
        }
        "aiuto" | "?" => stampa_aiuto(),
        altro => println!("  comando sconosciuto: `{altro}`. Scrivi `aiuto`."),
    }
    Comando::Niente
}

fn esegui_azione(azione: Azione, voce: &mut Voce, reg: &mut Registro, soglie: Soglie) {
    match azione {
        Azione::Deviazione { finestra } => {
            println!("  · fuori pista: {}", finestra.etichetta());
            reg.scrivi(Riga {
                tipo: "deviazione".into(),
                processo: Some(finestra.processo),
                titolo: Some(finestra.titolo),
                ..Default::default()
            });
        }
        Azione::Richiamo { livello, ritardo_ms, ripetizione } => {
            let esito = voce.richiama(livello);
            let (nome_esito, dettaglio) = match &esito {
                Esito::Suonato { file } => ("suonato", file.clone()),
                Esito::Ripiego { testo } => ("ripiego", testo.clone()),
                Esito::Muto => ("muto", String::new()),
            };
            match &esito {
                Esito::Ripiego { testo } => println!("  ! richiamo {}: {testo}", livello.numero()),
                _ => println!("  ! richiamo livello {} dopo {}", livello.numero(), durata_umana(ritardo_ms)),
            }
            reg.scrivi(Riga {
                tipo: "richiamo".into(),
                livello: Some(livello.numero()),
                ritardo_ms: Some(ritardo_ms),
                soglia_ms: Some(soglia_di(&soglie, livello)),
                ripetizione: Some(ripetizione),
                esito: Some(nome_esito.into()),
                nota: (!dettaglio.is_empty()).then_some(dettaglio),
                ..Default::default()
            });
        }
        Azione::Fine { durata_ms, ritorno_ms, livello_max, chiusura } => {
            if chiusura == Chiusura::Ritorno {
                match ritorno_ms {
                    Some(t) => println!("  · di nuovo in pista dopo {} dal richiamo", durata_umana(t)),
                    None => println!("  · di nuovo in pista ({} fuori, nessun richiamo)", durata_umana(durata_ms)),
                }
            }
            scrivi_fine(reg, durata_ms, ritorno_ms, livello_max, chiusura);
        }
        Azione::PausaFinita => {
            println!("  · pausa finita, cintura attiva.");
            reg.evento("pausa_finita");
        }
    }
}

fn scrivi_fine(
    reg: &mut Registro,
    durata_ms: u64,
    ritorno_ms: Option<u64>,
    livello_max: Option<Livello>,
    chiusura: Chiusura,
) {
    reg.scrivi(Riga {
        tipo: "fine_deviazione".into(),
        durata_ms: Some(durata_ms),
        ritorno_ms,
        livello: livello_max.map(|l| l.numero()),
        chiusura: Some(chiusura.come_stringa().to_string()),
        ..Default::default()
    });
}

fn soglia_di(soglie: &Soglie, livello: Livello) -> u64 {
    1000 * match livello {
        Livello::Suono => soglie.suono_s,
        Livello::Frase => soglie.frase_s,
        Livello::Dura => soglie.frase_dura_s,
    }
}

/// **Accettazione n. 4**: "il richiamo ti ha riportato in pista o l'hai
/// ignorato?". Non e' una domanda retorica e non la risponde un test: la fa la
/// cintura a fine sessione, e la risposta resta nel registro accanto ai numeri
/// che l'hanno provocata.
fn chiedi_verdetto(comandi: &mpsc::Receiver<String>, reg: &mut Registro) {
    println!("\nIl richiamo ti ha riportato in pista, o l'hai ignorato?");
    println!("(una riga; invio vuoto se non vuoi rispondere adesso)");
    print!("> ");
    usa_stdout();
    match comandi.recv_timeout(Duration::from_secs(180)) {
        Ok(risposta) if !risposta.trim().is_empty() => {
            reg.scrivi(Riga { tipo: "verdetto".into(), nota: Some(risposta.trim().into()), ..Default::default() });
            println!("  a registro.");
        }
        _ => println!("  nessun verdetto per questa sessione."),
    }
}

fn usa_stdout() {
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

/// `--nome valore` da una riga di comando scritta a mano. Sette opzioni non
/// valgono un parser di argomenti con le sue dipendenze.
fn valore_opzione(argomenti: &[String], nome: &str) -> Option<String> {
    let i = argomenti.iter().position(|a| a == nome)?;
    argomenti.get(i + 1).filter(|v| !v.starts_with("--")).cloned()
}

/// Ctrl+C: alza la bandiera e lascia che sia il ciclo a chiudere la sessione.
/// Il gestore gira su un thread di Windows — qui dentro si tocca **solo**
/// l'atomica, mai il registro.
#[cfg(target_os = "windows")]
fn registra_ctrl_c() {
    unsafe extern "system" fn gestore(_tipo: u32) -> i32 {
        USCITA.store(true, Ordering::Relaxed);
        1 // gestito: Windows non termina il processo
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<unsafe extern "system" fn(u32) -> i32>, add: i32) -> i32;
    }
    // SAFETY: si registra una funzione statica, senza stato catturato.
    unsafe {
        SetConsoleCtrlHandler(Some(gestore), 1);
    }
}

#[cfg(not(target_os = "windows"))]
fn registra_ctrl_c() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_opzioni_si_leggono_e_un_valore_mancante_non_ruba_l_opzione_dopo() {
        let a: Vec<String> = ["sessione", "--intento", "studio Rust", "--muto", "--dati"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(valore_opzione(&a, "--intento"), Some("studio Rust".into()));
        assert_eq!(valore_opzione(&a, "--dati"), None, "--dati senza valore non prende niente");
        assert_eq!(valore_opzione(&a, "--parole"), None);
    }

    #[test]
    fn un_opzione_non_si_mangia_quella_che_segue() {
        let a: Vec<String> = ["--intento", "--muto"].iter().map(|s| s.to_string()).collect();
        assert_eq!(valore_opzione(&a, "--intento"), None);
    }

    /// Un titolo che pulsa non deve riempire il registro.
    #[test]
    fn un_titolo_che_gira_da_solo_non_scrive_una_riga_al_secondo() {
        let a = finestra::Finestra {
            processo: "WindowsTerminal.exe".into(),
            titolo: "\u{25D1} build in corso".into(),
        };
        let b = finestra::Finestra { titolo: "\u{25D0} build in corso".into(), ..a.clone() };

        // Stesso processo, stesso verdetto, solo il titolo che pulsa: si tace...
        assert!(!merita_una_riga(&b, &a, Verdetto::InPista, Verdetto::InPista, 1_000, 0));
        // ...ma non per sempre: dopo il respiro la riga si riscrive.
        assert!(merita_una_riga(&b, &a, Verdetto::InPista, Verdetto::InPista, 6_000, 0));

        // Un cambio di processo o di verdetto passa sempre, e subito.
        let altro = finestra::Finestra { processo: "firefox.exe".into(), titolo: a.titolo.clone() };
        assert!(merita_una_riga(&altro, &a, Verdetto::InPista, Verdetto::InPista, 1_000, 0));
        assert!(merita_una_riga(&b, &a, Verdetto::FuoriPista, Verdetto::InPista, 1_000, 0));
    }

    #[test]
    fn ogni_livello_porta_la_sua_soglia_nel_registro() {
        let s = Soglie::default();
        assert_eq!(soglia_di(&s, Livello::Suono), 10_000);
        assert_eq!(soglia_di(&s, Livello::Frase), 45_000);
        assert_eq!(soglia_di(&s, Livello::Dura), 120_000);
    }
}
