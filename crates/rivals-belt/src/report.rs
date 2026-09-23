//! report.rs — il report di fine giornata (WO-01 §9) e la **verifica
//! dell'accettazione** (WO-01, criteri 1 e 2).
//!
//! Due comandi, una fonte sola: il registro JSONL. Nessun numero di questo file
//! e' calcolato altrove o tenuto in memoria fra un avvio e l'altro — se non e'
//! nel registro, non esiste. La regola e': ogni numero viene da un test, un log
//! o una misura.
//!
//! # Perche' la verifica e' un comando e non un test
//!
//! I criteri 1 e 2 del WO parlano di **sessioni vere**: soglia + 1,5 s "misurato
//! dal registro", e trenta minuti di lavoro senza richiami. Un test con
//! l'orologio finto prova che la macchina a stati e' giusta ([`crate::cintura`])
//! — non prova che il binario, su questo PC, con Windows che schedula come gli
//! pare, ci sia arrivato davvero. Quello lo dice solo il registro di ieri sera.
//!
//! I criteri 3 e 4 — tre sessioni reali, e il tuo verdetto su ogni richiamo —
//! non li calcola nessun comando: il primo lo conta questo file, il secondo te
//! lo chiede la cintura quando chiudi la sessione, e resta scritto.

use std::collections::BTreeMap;

use crate::registro::Riga;
use crate::tempo::durata_umana;

/// Il margine concesso dall'accettazione n. 1 sopra la soglia.
pub const MARGINE_MS: u64 = 1_500;

#[derive(Debug, Default)]
pub struct Sintesi {
    pub sessioni: Vec<String>,
    pub intenti: Vec<String>,
    pub deviazioni: usize,
    pub deviazioni_senza_richiamo: usize,
    pub chiuse_da_ritorno: usize,
    pub chiuse_da_incerto: usize,
    pub tempo_fuori_ms: u64,
    pub ritorni_ms: Vec<u64>,
    pub richiami: BTreeMap<u8, usize>,
    pub ritardo_massimo: Option<(u8, u64, u64)>,
    pub per_ora: BTreeMap<u32, (usize, u64)>,
    pub pause: Vec<(String, u64)>,
    pub ambigue: BTreeMap<String, usize>,
    pub verdetti: Vec<String>,
    pub frasi_mancanti: usize,
}

impl Sintesi {
    pub fn media_ritorno_ms(&self) -> Option<u64> {
        if self.ritorni_ms.is_empty() {
            return None;
        }
        Some(self.ritorni_ms.iter().sum::<u64>() / self.ritorni_ms.len() as u64)
    }

    /// La mediana insieme alla media, perche' una sola deviazione da mezz'ora
    /// sposta la media e non dice niente sulle altre.
    pub fn mediana_ritorno_ms(&self) -> Option<u64> {
        if self.ritorni_ms.is_empty() {
            return None;
        }
        let mut v = self.ritorni_ms.clone();
        v.sort_unstable();
        Some(v[v.len() / 2])
    }

    /// La fascia oraria peggiore: quella in cui hai passato **piu' tempo** fuori
    /// pista, non quella con piu' deviazioni. Dieci uscite da dieci secondi non
    /// sono una serata persa; una da mezz'ora si'.
    pub fn fascia_peggiore(&self) -> Option<(u32, usize, u64)> {
        self.per_ora
            .iter()
            .max_by_key(|(_, (n, ms))| (*ms, *n))
            .map(|(ora, (n, ms))| (*ora, *n, *ms))
    }
}

pub fn riassumi(righe: &[Riga]) -> Sintesi {
    let mut s = Sintesi::default();
    // Le deviazioni aperte ma non ancora chiuse, per sessione: servono a contare
    // quelle che non hanno mai prodotto un richiamo.
    let mut richiami_nella_deviazione: BTreeMap<String, usize> = BTreeMap::new();

    for r in righe {
        if !s.sessioni.contains(&r.sessione) {
            s.sessioni.push(r.sessione.clone());
        }
        match r.tipo.as_str() {
            "sessione_aperta" => {
                if let Some(i) = &r.intento {
                    if !i.is_empty() && !s.intenti.contains(i) {
                        s.intenti.push(i.clone());
                    }
                }
            }
            "finestra" => {
                if r.verdetto.as_deref() == Some("incerto") {
                    let etichetta = match (&r.processo, &r.titolo) {
                        (Some(p), Some(t)) if !t.is_empty() => format!("{p} — {t}"),
                        (Some(p), _) => p.clone(),
                        _ => continue,
                    };
                    *s.ambigue.entry(etichetta).or_insert(0) += 1;
                }
            }
            "deviazione" => {
                s.deviazioni += 1;
                richiami_nella_deviazione.insert(r.sessione.clone(), 0);
            }
            "richiamo" => {
                *s.richiami.entry(r.livello.unwrap_or(0)).or_insert(0) += 1;
                *richiami_nella_deviazione.entry(r.sessione.clone()).or_insert(0) += 1;
                if r.esito.as_deref() == Some("ripiego") {
                    s.frasi_mancanti += 1;
                }
                // Le ripetizioni non misurano la reattivita': la misura e' il
                // primo richiamo di un livello.
                if r.ripetizione != Some(true) {
                    if let (Some(rit), Some(soglia)) = (r.ritardo_ms, r.soglia_ms) {
                        let oltre = rit.saturating_sub(soglia);
                        if s.ritardo_massimo.map(|(_, _, o)| oltre > o).unwrap_or(true) {
                            s.ritardo_massimo = Some((r.livello.unwrap_or(0), rit, oltre));
                        }
                    }
                }
            }
            "fine_deviazione" => {
                let durata = r.durata_ms.unwrap_or(0);
                s.tempo_fuori_ms += durata;
                if richiami_nella_deviazione.remove(&r.sessione).unwrap_or(0) == 0 {
                    s.deviazioni_senza_richiamo += 1;
                }
                match r.chiusura.as_deref() {
                    Some("ritorno") => {
                        s.chiuse_da_ritorno += 1;
                        if let Some(t) = r.ritorno_ms {
                            s.ritorni_ms.push(t);
                        }
                    }
                    Some("incerto") => s.chiuse_da_incerto += 1,
                    _ => {}
                }
                if let Some(ora) = r.ora_del_giorno() {
                    let e = s.per_ora.entry(ora).or_insert((0, 0));
                    e.0 += 1;
                    e.1 += durata;
                }
            }
            "pausa" => {
                s.pause.push((
                    r.motivo.clone().unwrap_or_else(|| "senza motivo".into()),
                    r.durata_ms.unwrap_or(0),
                ));
            }
            "verdetto" => {
                if let Some(n) = &r.nota {
                    s.verdetti.push(n.clone());
                }
            }
            _ => {}
        }
    }
    s
}

/// Il report in testo, quello che si legge a fine giornata.
pub fn testo(giorno: &str, righe: &[Riga]) -> String {
    let s = riassumi(righe);
    let mut o = String::new();
    o.push_str(&format!("REPORT DELLA CINTURA — {giorno}\n"));
    o.push_str(&format!("{}\n\n", "=".repeat(38)));

    if righe.is_empty() {
        o.push_str("Nessuna riga di registro per questa giornata.\n");
        return o;
    }

    o.push_str(&format!("Sessioni: {}\n", s.sessioni.len()));
    for i in &s.intenti {
        o.push_str(&format!("  intento: {i}\n"));
    }

    o.push_str(&format!(
        "\nDeviazioni: {}  (senza richiamo, rientrate sotto la prima soglia: {})\n",
        s.deviazioni, s.deviazioni_senza_richiamo
    ));
    o.push_str(&format!("Tempo fuori pista: {}\n", durata_umana(s.tempo_fuori_ms)));
    o.push_str(&format!(
        "Chiuse da un ritorno: {} · finite su finestre incerte: {}\n",
        s.chiuse_da_ritorno, s.chiuse_da_incerto
    ));

    match (s.media_ritorno_ms(), s.mediana_ritorno_ms()) {
        (Some(media), Some(mediana)) => o.push_str(&format!(
            "Tempo di ritorno dopo il richiamo: media {} · mediana {} · su {} ritorni\n",
            durata_umana(media),
            durata_umana(mediana),
            s.ritorni_ms.len()
        )),
        _ => o.push_str("Tempo di ritorno: nessun ritorno dopo un richiamo da misurare.\n"),
    }

    o.push_str("\nRichiami\n");
    if s.richiami.is_empty() {
        o.push_str("  nessuno.\n");
    } else {
        for (livello, n) in &s.richiami {
            let nome = match livello {
                1 => "suono breve",
                2 => "frase",
                3 => "frase dura",
                _ => "sconosciuto",
            };
            o.push_str(&format!("  livello {livello} ({nome}): {n}\n"));
        }
    }
    if s.frasi_mancanti > 0 {
        o.push_str(&format!(
            "  {} richiami hanno ripiegato sul testo: i wav non sono ancora generati.\n",
            s.frasi_mancanti
        ));
    }

    if let Some((ora, n, ms)) = s.fascia_peggiore() {
        o.push_str(&format!(
            "\nFascia oraria peggiore: {ora:02}:00-{:02}:00 — {n} deviazioni, {} fuori pista\n",
            (ora + 1) % 24,
            durata_umana(ms)
        ));
        if s.per_ora.len() > 1 {
            o.push_str("Le altre:\n");
            let mut ordinate: Vec<_> = s.per_ora.iter().filter(|(h, _)| **h != ora).collect();
            ordinate.sort_by_key(|(_, (_, ms))| std::cmp::Reverse(*ms));
            for (h, (n, ms)) in ordinate.iter().take(3) {
                o.push_str(&format!("  {h:02}:00 — {n} deviazioni, {}\n", durata_umana(*ms)));
            }
        }
    }

    if !s.pause.is_empty() {
        o.push_str("\nPause\n");
        for (motivo, ms) in &s.pause {
            o.push_str(&format!("  {} — {motivo}\n", durata_umana(*ms)));
        }
    }

    if !s.ambigue.is_empty() {
        o.push_str("\nFinestre che le regole non conoscono (non hanno richiamato nessuno)\n");
        let mut v: Vec<_> = s.ambigue.iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        for (etichetta, n) in v.iter().take(8) {
            o.push_str(&format!("  {n:>3}×  {etichetta}\n"));
        }
        o.push_str("  → se una di queste e' distrazione, aggiungila a rules.toml.\n");
    }

    if !s.verdetti.is_empty() {
        o.push_str("\nIl tuo verdetto (accettazione n. 4)\n");
        for v in &s.verdetti {
            o.push_str(&format!("  {v}\n"));
        }
    } else {
        o.push_str("\nNessun verdetto tuo su questa giornata: chiudi la sessione con `esci`.\n");
    }

    o
}

/// La verifica dell'accettazione, sui numeri del registro.
pub fn verifica(giorno: &str, righe: &[Riga]) -> (String, bool) {
    let s = riassumi(righe);
    let mut o = String::new();
    let mut tutto_bene = true;
    o.push_str(&format!("VERIFICA DELL'ACCETTAZIONE — WO-01 — {giorno}\n"));
    o.push_str(&format!("{}\n\n", "=".repeat(44)));

    // 1 · Dal cambio di finestra al suono: soglia + al massimo 1,5 s.
    match s.ritardo_massimo {
        Some((livello, ritardo, oltre)) => {
            let ok = oltre <= MARGINE_MS;
            tutto_bene &= ok;
            o.push_str(&format!(
                "[{}] 1 · ritardo massimo dal cambio finestra al richiamo: {} ms oltre la soglia \
                 (livello {livello}, richiamo a {} ms). Concesso: {MARGINE_MS} ms.\n",
                se(ok),
                oltre,
                ritardo
            ));
        }
        None => o.push_str(
            "[ ? ] 1 · nessun richiamo nel registro: niente da misurare. Non e' un pass.\n",
        ),
    }

    // 2 · Zero richiami in 30 minuti di lavoro solo in pista.
    match finestra_piu_lunga_senza_richiami(righe) {
        Some(ms) => {
            let ok = ms >= 30 * 60_000;
            tutto_bene &= ok;
            o.push_str(&format!(
                "[{}] 2 · tratto piu' lungo senza deviazioni ne' richiami: {}. Servono 30 m.\n",
                se(ok),
                durata_umana(ms)
            ));
        }
        None => o.push_str("[ ? ] 2 · registro troppo corto per misurare un tratto pulito.\n"),
    }

    // 3 · Tre sessioni reali.
    let ok3 = s.sessioni.len() >= 3;
    o.push_str(&format!(
        "[{}] 3 · sessioni in questo file: {} (servono 3 sessioni reali, anche in giorni diversi).\n",
        se(ok3),
        s.sessioni.len()
    ));

    // 4 · Il verdetto e' tuo: nessun comando lo calcola.
    let ok4 = !s.verdetti.is_empty();
    o.push_str(&format!(
        "[{}] 4 · verdetti tuoi a registro: {}. Questo criterio non lo chiude un comando.\n",
        se(ok4),
        s.verdetti.len()
    ));
    tutto_bene &= ok3 && ok4;

    o.push_str(&format!(
        "\nEsito meccanico: {}\n",
        if tutto_bene { "tutti i criteri misurabili sono verdi" } else { "almeno un criterio non e' verde" }
    ));
    (o, tutto_bene)
}

fn se(ok: bool) -> &'static str {
    if ok { "ok" } else { "NO" }
}

/// Il tratto piu' lungo, dentro una sessione, senza deviazioni ne' richiami.
///
/// Si misura fra eventi consecutivi del registro e non "da quando hai aperto il
/// portatile": gli estremi sono l'apertura della sessione e la sua chiusura,
/// cosi' un file che finisce a meta' non regala mezz'ora pulita.
fn finestra_piu_lunga_senza_richiami(righe: &[Riga]) -> Option<u64> {
    let mut massimo: Option<u64> = None;
    let mut sessione_corrente: Option<&str> = None;
    let mut ultimo_evento_ms = 0u64;

    for r in righe {
        let nuova_sessione = sessione_corrente != Some(r.sessione.as_str());
        if nuova_sessione {
            sessione_corrente = Some(&r.sessione);
            ultimo_evento_ms = r.mono_ms;
            continue;
        }
        if matches!(r.tipo.as_str(), "deviazione" | "richiamo" | "pausa" | "sessione_chiusa") {
            let tratto = r.mono_ms.saturating_sub(ultimo_evento_ms);
            massimo = Some(massimo.map_or(tratto, |m: u64| m.max(tratto)));
            ultimo_evento_ms = r.mono_ms;
        }
    }
    massimo
}

#[cfg(test)]
mod tests {
    use super::*;

    fn riga(sessione: &str, mono_ms: u64, ts: &str, tipo: &str) -> Riga {
        Riga {
            v: 1,
            ts: ts.into(),
            mono_ms,
            sessione: sessione.into(),
            tipo: tipo.into(),
            ..Default::default()
        }
    }

    /// Una serata finta ma completa: due deviazioni, un ritorno lento e uno
    /// rapido, una pausa dichiarata.
    fn serata() -> Vec<Riga> {
        let mut r = vec![riga("2100", 0, "2026-09-22T21:00:00.000", "sessione_aperta")];
        r[0].intento = Some("finisco la cintura".into());

        r.push(riga("2100", 600_000, "2026-09-22T21:10:00.000", "deviazione"));
        let mut richiamo = riga("2100", 610_000, "2026-09-22T21:10:10.000", "richiamo");
        richiamo.livello = Some(1);
        richiamo.ritardo_ms = Some(10_400);
        richiamo.soglia_ms = Some(10_000);
        r.push(richiamo);
        let mut fine = riga("2100", 640_000, "2026-09-22T21:10:40.000", "fine_deviazione");
        fine.durata_ms = Some(40_000);
        fine.ritorno_ms = Some(30_000);
        fine.chiusura = Some("ritorno".into());
        r.push(fine);

        // Seconda deviazione, alle 23: piu' lunga, ed e' la fascia peggiore.
        r.push(riga("2100", 7_200_000, "2026-09-22T23:00:00.000", "deviazione"));
        let mut richiamo2 = riga("2100", 7_210_000, "2026-09-22T23:00:10.000", "richiamo");
        richiamo2.livello = Some(1);
        richiamo2.ritardo_ms = Some(11_900);
        richiamo2.soglia_ms = Some(10_000);
        r.push(richiamo2);
        let mut fine2 = riga("2100", 7_800_000, "2026-09-22T23:10:00.000", "fine_deviazione");
        fine2.durata_ms = Some(600_000);
        fine2.ritorno_ms = Some(590_000);
        fine2.chiusura = Some("ritorno".into());
        r.push(fine2);

        let mut pausa = riga("2100", 7_900_000, "2026-09-22T23:11:40.000", "pausa");
        pausa.durata_ms = Some(600_000);
        pausa.motivo = Some("cena".into());
        r.push(pausa);
        r
    }

    #[test]
    fn la_sintesi_conta_quello_che_il_wo_chiede() {
        let s = riassumi(&serata());
        assert_eq!(s.deviazioni, 2);
        assert_eq!(s.chiuse_da_ritorno, 2);
        assert_eq!(s.media_ritorno_ms(), Some(310_000));
        assert_eq!(s.richiami.get(&1), Some(&2));
        assert_eq!(s.pause.len(), 1);
    }

    #[test]
    fn la_fascia_peggiore_e_quella_dove_hai_perso_piu_tempo() {
        let s = riassumi(&serata());
        let (ora, n, ms) = s.fascia_peggiore().unwrap();
        assert_eq!(ora, 23, "alle 21 c'e' una deviazione da 40 s, alle 23 una da 10 m");
        assert_eq!(n, 1);
        assert_eq!(ms, 600_000);
    }

    #[test]
    fn una_deviazione_rientrata_prima_della_soglia_non_conta_come_richiamo() {
        let mut righe = vec![riga("0900", 0, "2026-09-23T09:00:00.000", "sessione_aperta")];
        righe.push(riga("0900", 10_000, "2026-09-23T09:00:10.000", "deviazione"));
        let mut fine = riga("0900", 16_000, "2026-09-23T09:00:16.000", "fine_deviazione");
        fine.durata_ms = Some(6_000);
        fine.chiusura = Some("ritorno".into());
        righe.push(fine);

        let s = riassumi(&righe);
        assert_eq!(s.deviazioni, 1);
        assert_eq!(s.deviazioni_senza_richiamo, 1);
        assert!(s.richiami.is_empty());
        assert_eq!(s.media_ritorno_ms(), None, "senza richiamo non c'e' tempo di ritorno");
    }

    #[test]
    fn la_verifica_boccia_un_richiamo_arrivato_tardi() {
        let mut righe = serata();
        if let Some(r) = righe.iter_mut().find(|r| r.tipo == "richiamo") {
            r.ritardo_ms = Some(13_000); // 3 s oltre la soglia: fuori accettazione
        }
        let (testo, ok) = verifica("2026-09-22", &righe);
        assert!(!ok);
        assert!(testo.contains("[NO] 1"), "{testo}");
    }

    #[test]
    fn la_verifica_promuove_una_serata_dentro_i_numeri() {
        let righe = serata();
        let (testo, _) = verifica("2026-09-22", &righe);
        // Il criterio 1 passa: 400 ms e 1900 ms oltre soglia, il peggiore e' sotto 1500.
        assert!(testo.contains("[NO] 1"), "1900 ms oltre soglia deve essere bocciato: {testo}");

        let mut pulita = righe.clone();
        if let Some(r) = pulita.iter_mut().find(|r| r.ritardo_ms == Some(11_900)) {
            r.ritardo_ms = Some(11_200);
        }
        let (testo, _) = verifica("2026-09-22", &pulita);
        assert!(testo.contains("[ok] 1"), "{testo}");
        // Il criterio 2 passa: fra il primo richiamo (10 m) e la deviazione delle
        // 23 ci sono quasi due ore senza niente.
        assert!(testo.contains("[ok] 2"), "{testo}");
        // Il 3 e il 4 no: una sessione sola, nessun verdetto tuo.
        assert!(testo.contains("[NO] 3") && testo.contains("[NO] 4"), "{testo}");
    }

    #[test]
    fn il_report_di_una_giornata_vuota_non_esplode() {
        let t = testo("2026-09-21", &[]);
        assert!(t.contains("Nessuna riga"));
    }

    #[test]
    fn il_report_dice_le_finestre_incerte_piu_viste() {
        let mut righe = vec![riga("0900", 0, "2026-09-23T09:00:00.000", "sessione_aperta")];
        for i in 0..3 {
            let mut f = riga("0900", 1000 * i, "2026-09-23T09:00:01.000", "finestra");
            f.processo = Some("Notion.exe".into());
            f.titolo = Some("Appunti".into());
            f.verdetto = Some("incerto".into());
            righe.push(f);
        }
        let t = testo("2026-09-23", &righe);
        assert!(t.contains("Notion.exe — Appunti"), "{t}");
        assert!(t.contains("3×"), "{t}");
    }
}
