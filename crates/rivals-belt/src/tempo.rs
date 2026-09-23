//! tempo.rs — due orologi, e non si mescolano.
//!
//! La cintura ne usa due, per due lavori diversi:
//!
//! - **il monotono** (`Istante`, da `std::time::Instant`) misura le durate: da
//!   quanto sei fuori pista, quanto ci hai messo a tornare. Non torna indietro
//!   se cambia l'ora legale e non salta se qualcuno sistema l'orologio.
//! - **l'ora locale** (`OraLocale`, da `GetLocalTime`) data le righe del
//!   registro e risponde alla domanda del report: *qual e' la tua fascia oraria
//!   peggiore?* Una fascia oraria in UTC non serve a nessuno.
//!
//! Le durate non si calcolano mai sottraendo due ore locali, e le fasce orarie
//! non si ricavano mai dal monotono. E' la ragione per cui ogni riga del
//! registro porta tutti e due i numeri: `ts` per leggerla, `mono_ms` per
//! misurarla.
//!
//! `GetLocalTime` invece di `chrono`: l'ora locale la sa gia' Windows, fuso e
//! ora legale compresi, e una dipendenza in meno e' una dipendenza in meno.

use std::time::Instant;

/// L'orologio monotono della sessione. Parte a zero all'avvio e conta
/// millisecondi: tutte le soglie della cintura sono in questa scala.
#[derive(Debug, Clone, Copy)]
pub struct Orologio {
    avvio: Instant,
}

impl Orologio {
    pub fn nuovo() -> Self {
        Self { avvio: Instant::now() }
    }

    /// Millisecondi dall'avvio della sessione.
    pub fn ms(&self) -> u64 {
        self.avvio.elapsed().as_millis() as u64
    }
}

/// Data e ora locali, come le vede Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OraLocale {
    pub anno: u16,
    pub mese: u16,
    pub giorno: u16,
    pub ora: u16,
    pub minuto: u16,
    pub secondo: u16,
    pub millis: u16,
}

impl OraLocale {
    /// `2026-09-22T21:04:07.123` — ordinabile come stringa, leggibile a occhio.
    ///
    /// Senza offset di fuso, di proposito: il registro e' locale per
    /// definizione, e un offset scritto e mai riletto e' solo rumore che un
    /// giorno qualcuno confrontera' con un altro fuso.
    pub fn iso(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}",
            self.anno, self.mese, self.giorno, self.ora, self.minuto, self.secondo, self.millis
        )
    }

    /// `2026-09-22` — la chiave con cui il report sceglie la giornata.
    pub fn giorno_iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.anno, self.mese, self.giorno)
    }
}

#[cfg(target_os = "windows")]
pub fn ora_locale() -> OraLocale {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        w_year: u16,
        w_month: u16,
        w_day_of_week: u16,
        w_day: u16,
        w_hour: u16,
        w_minute: u16,
        w_second: u16,
        w_milliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(st: *mut SystemTime);
    }

    let mut st = SystemTime::default();
    // SAFETY: `st` e' una SYSTEMTIME valida, allineata e viva per tutta la
    // chiamata; GetLocalTime la riempie e basta.
    unsafe { GetLocalTime(&mut st) };
    OraLocale {
        anno: st.w_year,
        mese: st.w_month,
        giorno: st.w_day,
        ora: st.w_hour,
        minuto: st.w_minute,
        secondo: st.w_second,
        millis: st.w_milliseconds,
    }
}

/// Fuori da Windows la cintura non gira — ma i test del report e della macchina
/// a stati devono compilare ovunque, quindi l'orologio esiste anche qui.
#[cfg(not(target_os = "windows"))]
pub fn ora_locale() -> OraLocale {
    let secondi = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    OraLocale {
        anno: 1970,
        mese: 1,
        giorno: 1,
        ora: ((secondi / 3600) % 24) as u16,
        minuto: ((secondi / 60) % 60) as u16,
        secondo: (secondi % 60) as u16,
        millis: 0,
    }
}

/// `1 h 04 m 09 s`, per il report. Sotto il minuto scrive i secondi con un
/// decimale: la differenza fra 8,4 e 12,7 secondi di ritorno conta.
pub fn durata_umana(ms: u64) -> String {
    let s = ms as f64 / 1000.0;
    if s < 60.0 {
        return format!("{s:.1} s");
    }
    let tot = ms / 1000;
    let (h, m, sec) = (tot / 3600, (tot % 3600) / 60, tot % 60);
    if h > 0 { format!("{h} h {m:02} m {sec:02} s") } else { format!("{m} m {sec:02} s") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_e_ordinabile_come_stringa() {
        let a = OraLocale { anno: 2026, mese: 9, giorno: 2, ora: 9, minuto: 5, secondo: 1, millis: 7 };
        let b = OraLocale { anno: 2026, mese: 9, giorno: 22, ora: 9, minuto: 5, secondo: 1, millis: 7 };
        assert_eq!(a.iso(), "2026-09-02T09:05:01.007");
        assert!(a.iso() < b.iso(), "lo zero davanti serve proprio a questo");
        assert_eq!(a.giorno_iso(), "2026-09-02");
    }

    #[test]
    fn durata_umana_cambia_scala_al_minuto() {
        assert_eq!(durata_umana(8_400), "8.4 s");
        assert_eq!(durata_umana(59_900), "59.9 s");
        assert_eq!(durata_umana(61_000), "1 m 01 s");
        assert_eq!(durata_umana(3_849_000), "1 h 04 m 09 s");
    }

    #[test]
    fn l_orologio_monotono_non_torna_indietro() {
        let o = Orologio::nuovo();
        let a = o.ms();
        let b = o.ms();
        assert!(b >= a);
    }
}
