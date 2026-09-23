//! cintura.rs — **N2, la cintura** (WO-01 §5): la macchina a stati che decide
//! *quando* si richiama, e con quanta forza.
//!
//! E' una macchina a stati **pura**: entra un
//! istante piu' un verdetto, escono delle azioni. Non legge l'orologio, non
//! apre file, non suona niente. E' questo che la rende provabile a tavolino —
//! mezz'ora di lavoro, due ore di distrazione e una pausa si simulano in
//! millisecondi, e l'accettazione n. 1 del WO (dal cambio finestra al suono:
//! soglia + al massimo 1,5 s) diventa un test invece di un cronometro in mano.
//!
//! # I tre principi che il codice qui sotto deve rispettare
//!
//! - **Il richiamo e' neutro.** Nessun giudizio, nessuna persona: la cintura
//!   non sa chi sei, sa solo che questa finestra non c'entra con quello che hai
//!   detto di fare. I rivali, che giudicano, arrivano dal WO-06 e passano dal
//!   regista — mai da qui.
//! - **Si ferma appena torni.** Non c'e' coda, non c'e' "ma prima ti dico
//!   l'ultima": il ritorno in pista spegne l'escalation nello stesso tick.
//! - **L'incerto non richiama.** Lo decide [`crate::regole`], qui se ne
//!   raccolgono le conseguenze: incerto e in pista chiudono tutti e due la
//!   deviazione, ma solo uno dei due e' un ritorno.

use crate::finestra::Finestra;
use crate::regole::{Soglie, Verdetto};

/// Il livello dell'escalation. I numeri sono quelli del §5 e finiscono nel
/// registro: cambiarli e' un cambio di formato, non un dettaglio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Livello {
    /// Oltre la prima soglia: un suono breve, niente parole.
    Suono = 1,
    /// Oltre la seconda: una frase registrata.
    Frase = 2,
    /// Oltre la terza: una frase piu' dura, ripetuta finche' non torni.
    Dura = 3,
}

impl Livello {
    pub fn numero(self) -> u8 {
        self as u8
    }
}

/// Perche' una deviazione si e' chiusa. La distinzione non e' burocrazia: il
/// report calcola il **tempo medio di ritorno** solo sulle deviazioni chiuse da
/// un ritorno vero. Una chiusa dalla pausa direbbe che la cintura ha funzionato
/// quando invece l'hai messa a tacere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chiusura {
    /// Sei tornato su una finestra in pista.
    Ritorno,
    /// Sei finito su una finestra che le regole non conoscono.
    Incerto,
    /// Hai messo in pausa mentre eri fuori pista.
    Pausa,
    /// La sessione e' finita mentre eri fuori pista.
    FineSessione,
}

impl Chiusura {
    pub fn come_stringa(self) -> &'static str {
        match self {
            Chiusura::Ritorno => "ritorno",
            Chiusura::Incerto => "incerto",
            Chiusura::Pausa => "pausa",
            Chiusura::FineSessione => "fine_sessione",
        }
    }
}

/// Cosa la cintura chiede di fare al mondo di fuori. Tutto quello che esce da
/// qui va **sia** eseguito **sia** messo a registro: il registro e' l'unica
/// prova che l'accettazione e' rispettata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Azione {
    /// Sei uscito di pista adesso. Nessun suono: il primo richiamo arriva alla
    /// prima soglia, non subito.
    Deviazione { finestra: Finestra },
    /// Richiamo. `ritardo_ms` e' il tempo passato da quando la cintura ha
    /// **visto** la deviazione: e' il numero dell'accettazione n. 1.
    Richiamo { livello: Livello, ritardo_ms: u64, ripetizione: bool },
    /// Deviazione chiusa. `ritorno_ms` c'e' solo se un richiamo era partito:
    /// e' il tempo che il richiamo ha impiegato a riportarti indietro.
    Fine {
        durata_ms: u64,
        ritorno_ms: Option<u64>,
        livello_max: Option<Livello>,
        chiusura: Chiusura,
    },
    /// La pausa e' scaduta da sola. La cintura riprende a guardare.
    PausaFinita,
}

#[derive(Debug, Clone)]
enum Stato {
    InPista,
    Fuori {
        /// Quando la cintura **ha visto** la deviazione, non quando e' iniziata
        /// davvero: fra le due c'e' al massimo un intervallo di polling, ed e'
        /// il motivo per cui l'accettazione concede 1,5 s sopra la soglia.
        inizio_ms: u64,
        livello: Option<Livello>,
        primo_richiamo_ms: Option<u64>,
        /// Quando ripetere, al livello 3.
        prossima_ripetizione_ms: u64,
        finestra: Finestra,
    },
    Pausa {
        fino_ms: u64,
        motivo: String,
    },
}

pub struct Cintura {
    soglie: Soglie,
    stato: Stato,
}

impl Cintura {
    pub fn nuova(soglie: Soglie) -> Self {
        Self { soglie, stato: Stato::InPista }
    }

    pub fn in_pausa(&self) -> bool {
        matches!(self.stato, Stato::Pausa { .. })
    }

    /// Da quanti millisecondi sei fuori pista, se lo sei.
    pub fn fuori_da(&self, ora_ms: u64) -> Option<u64> {
        match &self.stato {
            Stato::Fuori { inizio_ms, .. } => Some(ora_ms.saturating_sub(*inizio_ms)),
            _ => None,
        }
    }

    pub fn motivo_pausa(&self) -> Option<&str> {
        match &self.stato {
            Stato::Pausa { motivo, .. } => Some(motivo.as_str()),
            _ => None,
        }
    }

    /// **Il cuore.** Un tick: ecco l'ora, ecco la finestra, ecco il verdetto.
    pub fn osserva(&mut self, ora_ms: u64, finestra: &Finestra, verdetto: Verdetto) -> Vec<Azione> {
        let mut azioni = Vec::new();

        // La pausa e' cieca per definizione: finche' dura, la cintura non
        // guarda e non conta. Scaduta, riparte dallo stato pulito.
        if let Stato::Pausa { fino_ms, .. } = self.stato {
            if ora_ms < fino_ms {
                return azioni;
            }
            self.stato = Stato::InPista;
            azioni.push(Azione::PausaFinita);
        }

        match verdetto {
            Verdetto::FuoriPista => self.tick_fuori(ora_ms, finestra, &mut azioni),
            Verdetto::InPista => {
                if let Some(fine) = self.chiudi(ora_ms, Chiusura::Ritorno) {
                    azioni.push(fine);
                }
            }
            Verdetto::Incerto => {
                // Cambiare da YouTube a una finestra che non conosciamo non e'
                // tornare al lavoro — ma nemmeno restare fuori pista, e nel
                // dubbio la cintura tace. La deviazione si chiude, marcata
                // `incerto`: il report la conta a parte, cosi' il numero delle
                // deviazioni vere non si gonfia.
                if let Some(fine) = self.chiudi(ora_ms, Chiusura::Incerto) {
                    azioni.push(fine);
                }
            }
        }
        azioni
    }

    fn tick_fuori(&mut self, ora_ms: u64, finestra: &Finestra, azioni: &mut Vec<Azione>) {
        if let Stato::InPista = self.stato {
            self.stato = Stato::Fuori {
                inizio_ms: ora_ms,
                livello: None,
                primo_richiamo_ms: None,
                prossima_ripetizione_ms: 0,
                finestra: finestra.clone(),
            };
            azioni.push(Azione::Deviazione { finestra: finestra.clone() });
        }

        let Stato::Fuori { inizio_ms, livello, primo_richiamo_ms, prossima_ripetizione_ms, finestra: corrente } =
            &mut self.stato
        else {
            return;
        };

        // Passare da YouTube a Reddit **non** azzera il cronometro: e' la stessa
        // deviazione che continua. Si aggiorna solo l'etichetta, perche' il
        // registro deve dire dove sei adesso.
        if !corrente.e_lo_stesso(finestra) {
            *corrente = finestra.clone();
        }

        let fuori_da = ora_ms.saturating_sub(*inizio_ms);
        let atteso = livello_atteso(&self.soglie, fuori_da);

        match (atteso, *livello) {
            (Some(nuovo), attuale) if Some(nuovo) > attuale => {
                *livello = Some(nuovo);
                if primo_richiamo_ms.is_none() {
                    *primo_richiamo_ms = Some(ora_ms);
                }
                if nuovo == Livello::Dura {
                    *prossima_ripetizione_ms = ora_ms + self.soglie.ripetizione_s * 1000;
                }
                azioni.push(Azione::Richiamo { livello: nuovo, ritardo_ms: fuori_da, ripetizione: false });
            }
            (Some(Livello::Dura), Some(Livello::Dura)) if ora_ms >= *prossima_ripetizione_ms => {
                *prossima_ripetizione_ms = ora_ms + self.soglie.ripetizione_s * 1000;
                azioni.push(Azione::Richiamo {
                    livello: Livello::Dura,
                    ritardo_ms: fuori_da,
                    ripetizione: true,
                });
            }
            _ => {}
        }
    }

    /// Mette in pausa fino a `fino_ms`. Se eri fuori pista, la deviazione si
    /// chiude qui e la ritrovi nel registro col suo motivo: **fermarsi si puo',
    /// ma resta scritto**.
    pub fn pausa(&mut self, ora_ms: u64, durata_ms: u64, motivo: &str) -> Vec<Azione> {
        let mut azioni = Vec::new();
        if let Some(fine) = self.chiudi(ora_ms, Chiusura::Pausa) {
            azioni.push(fine);
        }
        self.stato = Stato::Pausa { fino_ms: ora_ms + durata_ms, motivo: motivo.to_string() };
        azioni
    }

    /// Torna a guardare prima che la pausa scada.
    pub fn riprendi(&mut self) {
        if self.in_pausa() {
            self.stato = Stato::InPista;
        }
    }

    /// Fine sessione: se c'era una deviazione aperta la si chiude, altrimenti
    /// resterebbe fuori dal report.
    pub fn chiudi_sessione(&mut self, ora_ms: u64) -> Option<Azione> {
        self.chiudi(ora_ms, Chiusura::FineSessione)
    }

    fn chiudi(&mut self, ora_ms: u64, chiusura: Chiusura) -> Option<Azione> {
        let Stato::Fuori { inizio_ms, livello, primo_richiamo_ms, .. } = &self.stato else {
            return None;
        };
        let durata_ms = ora_ms.saturating_sub(*inizio_ms);
        // Il tempo di ritorno si misura dal **primo richiamo**, non dall'inizio
        // della deviazione: la domanda a cui il report risponde e' "quanto ci
        // mette il richiamo a riportarti indietro", e prima del richiamo la
        // cintura non aveva ancora aperto bocca.
        let ritorno_ms = match (chiusura, primo_richiamo_ms) {
            (Chiusura::Ritorno, Some(t)) => Some(ora_ms.saturating_sub(*t)),
            _ => None,
        };
        let azione = Azione::Fine { durata_ms, ritorno_ms, livello_max: *livello, chiusura };
        self.stato = Stato::InPista;
        Some(azione)
    }
}

/// A che livello *dovrebbe* essere l'escalation dopo tanti millisecondi fuori
/// pista. Si confronta col livello gia' raggiunto: la cintura non scende mai da
/// sola, scende solo tornando in pista.
fn livello_atteso(soglie: &Soglie, fuori_da_ms: u64) -> Option<Livello> {
    let s = fuori_da_ms / 1000;
    if s >= soglie.frase_dura_s {
        Some(Livello::Dura)
    } else if s >= soglie.frase_s {
        Some(Livello::Frase)
    } else if s >= soglie.suono_s {
        Some(Livello::Suono)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn soglie() -> Soglie {
        Soglie::default() // 1000 ms · 10 s · 45 s · 120 s · ogni 60 s
    }

    fn youtube() -> Finestra {
        Finestra { processo: "firefox.exe".into(), titolo: "Lofi beats - YouTube".into() }
    }

    fn editor() -> Finestra {
        Finestra { processo: "Code.exe".into(), titolo: "cintura.rs".into() }
    }

    /// Fa girare la cintura come farebbe il ciclo vero: un tick ogni
    /// `intervallo_ms`, da 0 a `fino_ms`, sempre sullo stesso verdetto.
    fn gira(c: &mut Cintura, da_ms: u64, fino_ms: u64, f: &Finestra, v: Verdetto) -> Vec<(u64, Azione)> {
        let passo = soglie().intervallo_ms;
        let mut fuori = Vec::new();
        let mut t = da_ms;
        while t <= fino_ms {
            for a in c.osserva(t, f, v) {
                fuori.push((t, a));
            }
            t += passo;
        }
        fuori
    }

    #[test]
    fn sotto_la_prima_soglia_la_cintura_tace() {
        let mut c = Cintura::nuova(soglie());
        let azioni = gira(&mut c, 0, 9_000, &youtube(), Verdetto::FuoriPista);
        assert_eq!(azioni.len(), 1, "solo l'apertura della deviazione: {azioni:?}");
        assert!(matches!(azioni[0].1, Azione::Deviazione { .. }));
    }

    /// **Accettazione n. 1 del WO-01.** Dal momento in cui la cintura vede la
    /// finestra fuori pista al suono passano al massimo soglia + 1,5 s, e il
    /// numero e' quello che finisce nel registro.
    #[test]
    fn dal_cambio_finestra_al_suono_soglia_piu_al_massimo_un_secondo_e_mezzo() {
        for intervallo_ms in [200, 500, 1000, 1500] {
            let mut s = soglie();
            s.intervallo_ms = intervallo_ms;
            let mut c = Cintura::nuova(s);

            let mut t = 0;
            let mut ritardo = None;
            while t <= 30_000 && ritardo.is_none() {
                for a in c.osserva(t, &youtube(), Verdetto::FuoriPista) {
                    if let Azione::Richiamo { livello: Livello::Suono, ritardo_ms, .. } = a {
                        ritardo = Some(ritardo_ms);
                    }
                }
                t += intervallo_ms;
            }

            let r = ritardo.expect("il suono deve partire");
            assert!(r >= s.suono_s * 1000, "mai prima della soglia: {r} ms");
            assert!(
                r <= s.suono_s * 1000 + 1_500,
                "intervallo {intervallo_ms} ms: ritardo {r} ms, oltre soglia + 1,5 s"
            );
        }
    }

    #[test]
    fn l_escalation_sale_di_un_gradino_alla_volta_e_poi_si_ripete() {
        let mut c = Cintura::nuova(soglie());
        let azioni = gira(&mut c, 0, 250_000, &youtube(), Verdetto::FuoriPista);
        let richiami: Vec<_> = azioni
            .iter()
            .filter_map(|(t, a)| match a {
                Azione::Richiamo { livello, ripetizione, .. } => Some((*t, *livello, *ripetizione)),
                _ => None,
            })
            .collect();

        assert_eq!(richiami[0], (10_000, Livello::Suono, false));
        assert_eq!(richiami[1], (45_000, Livello::Frase, false));
        assert_eq!(richiami[2], (120_000, Livello::Dura, false));
        assert_eq!(richiami[3], (180_000, Livello::Dura, true), "ogni 60 s, e marcata come ripetizione");
        assert_eq!(richiami[4], (240_000, Livello::Dura, true));
        assert_eq!(richiami.len(), 5, "niente richiami di troppo: {richiami:?}");
    }

    #[test]
    fn il_ritorno_spegne_tutto_nello_stesso_tick() {
        let mut c = Cintura::nuova(soglie());
        gira(&mut c, 0, 50_000, &youtube(), Verdetto::FuoriPista);

        let azioni = c.osserva(51_000, &editor(), Verdetto::InPista);
        match &azioni[..] {
            [Azione::Fine { durata_ms, ritorno_ms, livello_max, chiusura }] => {
                assert_eq!(*durata_ms, 51_000);
                // Primo richiamo a 10 s, ritorno a 51 s: 41 s per riportarti indietro.
                assert_eq!(*ritorno_ms, Some(41_000));
                assert_eq!(*livello_max, Some(Livello::Frase));
                assert_eq!(*chiusura, Chiusura::Ritorno);
            }
            altro => panic!("il ritorno deve chiudere e basta: {altro:?}"),
        }
        assert!(c.fuori_da(51_000).is_none(), "la deviazione e' chiusa");
        // E dopo il ritorno non deve restare niente in canna.
        assert!(gira(&mut c, 52_000, 400_000, &editor(), Verdetto::InPista).is_empty());
    }

    #[test]
    fn cambiare_finestra_fuori_pista_non_azzera_il_cronometro() {
        let mut c = Cintura::nuova(soglie());
        gira(&mut c, 0, 8_000, &youtube(), Verdetto::FuoriPista);

        let reddit = Finestra { processo: "firefox.exe".into(), titolo: "r/rust - Reddit".into() };
        let azioni = gira(&mut c, 9_000, 12_000, &reddit, Verdetto::FuoriPista);

        let richiami: Vec<_> = azioni.iter().filter(|(_, a)| matches!(a, Azione::Richiamo { .. })).collect();
        assert_eq!(richiami.len(), 1, "un solo suono, alla soglia di sempre");
        assert!(
            matches!(richiami[0], (10_000, Azione::Richiamo { ritardo_ms: 10_000, .. })),
            "il cronometro parte dalla prima finestra fuori pista, non dall'ultima: {richiami:?}"
        );
    }

    #[test]
    fn l_incerto_chiude_la_deviazione_e_non_richiama_mai() {
        let mut c = Cintura::nuova(soglie());
        gira(&mut c, 0, 20_000, &youtube(), Verdetto::FuoriPista);

        let sconosciuta = Finestra { processo: "explorer.exe".into(), titolo: "Download".into() };
        let azioni = c.osserva(21_000, &sconosciuta, Verdetto::Incerto);
        match &azioni[..] {
            [Azione::Fine { ritorno_ms, chiusura, .. }] => {
                assert_eq!(*chiusura, Chiusura::Incerto);
                assert!(ritorno_ms.is_none(), "l'incerto non e' un ritorno, e non entra nella media");
            }
            altro => panic!("{altro:?}"),
        }
        // E un'ora di finestre sconosciute non produce un solo richiamo.
        assert!(gira(&mut c, 22_000, 3_622_000, &sconosciuta, Verdetto::Incerto).is_empty());
    }

    #[test]
    fn la_pausa_e_cieca_e_scade_da_sola() {
        let mut c = Cintura::nuova(soglie());
        gira(&mut c, 0, 60_000, &youtube(), Verdetto::FuoriPista);

        let chiusura = c.pausa(61_000, 10 * 60_000, "caffe' e telefonata");
        assert!(matches!(chiusura[..], [Azione::Fine { chiusura: Chiusura::Pausa, .. }]));
        assert_eq!(c.motivo_pausa(), Some("caffe' e telefonata"));

        // Dieci minuti su YouTube in pausa: nemmeno un richiamo.
        assert!(gira(&mut c, 62_000, 660_000, &youtube(), Verdetto::FuoriPista).is_empty());

        // Scaduta, riparte da zero: la prima deviazione dopo la pausa e' nuova.
        let dopo = gira(&mut c, 661_000, 680_000, &youtube(), Verdetto::FuoriPista);
        assert!(matches!(dopo[0].1, Azione::PausaFinita));
        assert!(matches!(dopo[1].1, Azione::Deviazione { .. }));
        let primo_richiamo = dopo
            .iter()
            .find_map(|(t, a)| matches!(a, Azione::Richiamo { .. }).then_some(*t))
            .expect("dopo la pausa la cintura riprende a richiamare");
        assert_eq!(primo_richiamo, 671_000, "10 s dopo la fine della pausa, non 10 s dopo l'inizio");
    }

    /// **Accettazione n. 2 del WO-01**: zero richiami in 30 minuti di lavoro
    /// solo in pista. Qui sono 30 minuti simulati a 1 Hz, 1800 tick.
    #[test]
    fn trenta_minuti_in_pista_fanno_zero_richiami() {
        let mut c = Cintura::nuova(soglie());
        let azioni = gira(&mut c, 0, 30 * 60_000, &editor(), Verdetto::InPista);
        assert!(azioni.is_empty(), "silenzio assoluto: {azioni:?}");
    }

    #[test]
    fn la_sessione_che_finisce_fuori_pista_non_perde_la_deviazione() {
        let mut c = Cintura::nuova(soglie());
        gira(&mut c, 0, 30_000, &youtube(), Verdetto::FuoriPista);
        match c.chiudi_sessione(31_000) {
            Some(Azione::Fine { durata_ms, chiusura: Chiusura::FineSessione, .. }) => {
                assert_eq!(durata_ms, 31_000);
            }
            altro => panic!("{altro:?}"),
        }
        assert!(c.chiudi_sessione(32_000).is_none(), "non si chiude due volte");
    }
}
