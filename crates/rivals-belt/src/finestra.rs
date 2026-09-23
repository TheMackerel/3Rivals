//! finestra.rs — **N1, il sensore finestre** (WO-01 §2).
//!
//! Una volta al secondo: qual e' la finestra in primo piano, come si chiama il
//! processo che la possiede, cosa c'e' scritto nella barra del titolo. Nient'altro.
//! Niente screenshot, niente lettura del contenuto, niente accessibility API:
//! il WO mette la visione fuori dall'MVP, e qui il confine e' il codice, non una
//! promessa nel README.
//!
//! Il titolo del browser basta perche' contiene gia' il titolo della scheda:
//! `Rust concurrency tutorial - YouTube — Mozilla Firefox`. E' il motivo per cui
//! la cintura non ha bisogno di un'estensione del browser per sapere cosa stai
//! guardando — e per cui le parole dell'intento (→ [`crate::regole`]) possono
//! sbloccare un titolo che a prima vista e' fuori pista.
//!
//! FFI raw a `user32`/`kernel32`: sei funzioni non giustificano il crate
//! `windows`.

/// Cosa hai davanti in questo istante.
///
/// `processo` e' il solo nome del file (`firefox.exe`), non il percorso: le
/// regole si scrivono sul nome, e il percorso pieno finirebbe nel registro —
/// che e' un file che poi si legge in giro.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Finestra {
    pub processo: String,
    pub titolo: String,
}

impl Finestra {
    /// Due finestre sono "la stessa" se coincidono processo e titolo: e' il
    /// cambio di **cosa stai facendo**, non di quale HWND ha il fuoco. Cambiare
    /// scheda dentro Firefox cambia il titolo, quindi conta come cambio.
    pub fn e_lo_stesso(&self, altra: &Finestra) -> bool {
        self.processo == altra.processo && self.titolo == altra.titolo
    }

    /// Riga corta per il registro e per il report.
    pub fn etichetta(&self) -> String {
        if self.titolo.is_empty() {
            self.processo.clone()
        } else {
            format!("{} — {}", self.processo, self.titolo)
        }
    }
}

#[cfg(target_os = "windows")]
mod win {
    use std::os::raw::c_void;

    pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn GetForegroundWindow() -> *mut c_void;
        pub fn GetWindowTextW(hwnd: *mut c_void, buf: *mut u16, max: i32) -> i32;
        pub fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        pub fn QueryFullProcessImageNameW(
            proc: *mut c_void,
            flags: u32,
            buf: *mut u16,
            size: *mut u32,
        ) -> i32;
        pub fn CloseHandle(h: *mut c_void) -> i32;
    }
}

/// La finestra attiva adesso, oppure `None`.
///
/// `None` non e' un errore: e' lo schermo bloccato, il desktop nudo, o il mezzo
/// secondo in cui Windows sta passando il fuoco da una finestra all'altra. La
/// cintura lo tratta come **incerto**, non come fuori pista — un richiamo alla
/// schermata di blocco sarebbe esattamente il falso positivo che il WO dice di
/// evitare.
#[cfg(target_os = "windows")]
pub fn finestra_attiva() -> Option<Finestra> {
    // SAFETY: nessun puntatore nostro attraversa il confine; gli HWND e gli
    // handle li controlliamo prima di usarli, e l'handle di processo si chiude
    // su ogni ramo.
    unsafe {
        let hwnd = win::GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        let mut buf = [0u16; 512];
        let n = win::GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        let titolo = if n > 0 { String::from_utf16_lossy(&buf[..n as usize]) } else { String::new() };

        let mut pid: u32 = 0;
        win::GetWindowThreadProcessId(hwnd, &mut pid);
        let processo = if pid != 0 { nome_processo(pid) } else { String::new() };

        if processo.is_empty() && titolo.is_empty() {
            return None;
        }
        Some(Finestra { processo, titolo })
    }
}

/// Il nome del file eseguibile di un pid, senza percorso.
///
/// `PROCESS_QUERY_LIMITED_INFORMATION` e non `PROCESS_QUERY_INFORMATION`: il
/// primo funziona anche su processi di integrita' piu' alta senza chiedere
/// privilegi, ed e' tutto quello che serve per leggere un nome. La cintura non
/// deve mai avere bisogno di girare da amministratore.
#[cfg(target_os = "windows")]
fn nome_processo(pid: u32) -> String {
    unsafe {
        let h = win::OpenProcess(win::PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            // Succede: processi di sistema, o processo morto fra la GetWindow e qui.
            return String::new();
        }
        let mut buf = [0u16; 260];
        let mut size = buf.len() as u32;
        let ok = win::QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut size);
        win::CloseHandle(h);
        if ok == 0 {
            return String::new();
        }
        let intero = String::from_utf16_lossy(&buf[..size as usize]);
        intero.rsplit(['\\', '/']).next().unwrap_or("").to_string()
    }
}

/// Fuori da Windows la cintura non ha occhi. Esiste per far compilare i test
/// della macchina a stati, che di occhi non hanno bisogno.
#[cfg(not(target_os = "windows"))]
pub fn finestra_attiva() -> Option<Finestra> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn il_cambio_scheda_e_un_cambio_finestra() {
        let a = Finestra { processo: "firefox.exe".into(), titolo: "docs.rs — Firefox".into() };
        let b = Finestra { processo: "firefox.exe".into(), titolo: "YouTube — Firefox".into() };
        assert!(!a.e_lo_stesso(&b));
        assert!(a.e_lo_stesso(&a.clone()));
    }

    #[test]
    fn l_etichetta_regge_un_titolo_vuoto() {
        let f = Finestra { processo: "explorer.exe".into(), titolo: String::new() };
        assert_eq!(f.etichetta(), "explorer.exe");
    }

    /// Sanity del sensore vero: su Windows deve rispondere senza esplodere.
    /// Non asserisce *cosa* vede — dipende da chi lancia i test — ma che il
    /// giro di FFI sia sano: nessun panico, nessun titolo spazzatura.
    #[cfg(target_os = "windows")]
    #[test]
    fn il_sensore_risponde_senza_esplodere() {
        if let Some(f) = finestra_attiva() {
            assert!(!f.processo.contains('\\'), "il processo e' solo il nome del file");
            assert!(f.titolo.len() < 1024);
        }
    }
}
