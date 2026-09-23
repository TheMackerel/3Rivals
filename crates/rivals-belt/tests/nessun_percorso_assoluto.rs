//! Nessun percorso della macchina di sviluppo nel sorgente.
//!
//! Un percorso assoluto scritto nel codice funziona su un PC solo, e se ne
//! accorge chi lo installa, non chi lo scrive. Qui e' un test rosso dal primo
//! giorno, e i percorsi si scoprono a runtime in `percorsi.rs`.
//!
//! # Come si dichiara un'eccezione
//!
//! Una riga puo' contenere un percorso assoluto solo se porta, sulla stessa
//! riga, il marcatore `percorso di prova`. Serve ai test che costruiscono
//! percorsi finti. Un marcatore su una riga di codice vero e' un imbroglio che
//! si vede in review: sono due parole, e stanno accanto alla cosa che scusano.

use std::path::{Path, PathBuf};

const MARCATORE: &str = "percorso di prova";

#[test]
fn nessun_percorso_assoluto_nel_sorgente() {
    let radice = radice_dei_crate();
    let mut file_visti = 0usize;
    let mut colpevoli: Vec<String> = Vec::new();

    for file in file_rust(&radice) {
        file_visti += 1;
        let testo = std::fs::read_to_string(&file).unwrap_or_default();
        for (n, riga) in testo.lines().enumerate() {
            let pulita = riga.trim_start();
            // I commenti possono citare il problema — questo file lo fa due volte.
            if pulita.starts_with("//") || riga.contains(MARCATORE) {
                continue;
            }
            if let Some(trovato) = percorso_assoluto(riga) {
                colpevoli.push(format!(
                    "{}:{} → {trovato}",
                    file.strip_prefix(&radice).unwrap_or(&file).display(),
                    n + 1
                ));
            }
        }
    }

    assert!(file_visti >= 8, "il test deve vedere il sorgente: {file_visti} file trovati sotto {}", radice.display());
    assert!(
        colpevoli.is_empty(),
        "percorsi assoluti nel sorgente ({} righe):\n  {}\n\nI percorsi si scoprono a runtime → src/percorsi.rs",
        colpevoli.len(),
        colpevoli.join("\n  ")
    );
}

/// `C:\`, `C:/`, `/home/`, `/Users/`, `\Users\` — le forme con cui un percorso
/// di sviluppo entra in un repo.
fn percorso_assoluto(riga: &str) -> Option<String> {
    let b: Vec<char> = riga.chars().collect();
    for i in 0..b.len().saturating_sub(2) {
        if b[i].is_ascii_alphabetic() && b[i + 1] == ':' && (b[i + 2] == '\\' || b[i + 2] == '/') {
            // `http://` e `file://` non sono percorsi di macchina: la lettera
            // prima dei due punti fa parte di una parola piu' lunga.
            let precedente = if i == 0 { ' ' } else { b[i - 1] };
            if !precedente.is_ascii_alphanumeric() {
                return Some(b[i..(i + 3).min(b.len())].iter().collect());
            }
        }
    }
    for fisso in ["/home/", "/Users/", "\\Users\\"] { // percorso di prova: sono i modelli cercati
        if riga.contains(fisso) {
            return Some(fisso.to_string());
        }
    }
    None
}

fn radice_dei_crate() -> PathBuf {
    // CARGO_MANIFEST_DIR e' l'unico percorso assoluto ammesso: lo mette cargo,
    // non noi, e cambia con la macchina — che e' esattamente il punto.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}

fn file_rust(dir: &Path) -> Vec<PathBuf> {
    let mut fuori = Vec::new();
    let Ok(voci) = std::fs::read_dir(dir) else {
        return fuori;
    };
    for voce in voci.flatten() {
        let p = voce.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            fuori.extend(file_rust(&p));
        } else if p.extension().is_some_and(|e| e == "rs") {
            fuori.push(p);
        }
    }
    fuori
}

#[test]
fn il_riconoscitore_riconosce_quello_che_deve() {
    // Ogni riga qui sotto porta il marcatore: sono i casi che il test deve
    // riconoscere, e senza marcatore il test si boccerebbe da solo.
    assert!(percorso_assoluto(r#""C:\Users\nome\modelli""#).is_some()); // percorso di prova
    assert!(percorso_assoluto("\"C:/Users/nome/modelli\"").is_some()); // percorso di prova
    assert!(percorso_assoluto("\"/home/utente/.rivals\"").is_some()); // percorso di prova
    assert!(percorso_assoluto("\"D:/Modelli/kokoro.onnx\"").is_some()); // percorso di prova

    assert!(percorso_assoluto("let url = \"https://docs.rs/serde\";").is_none(), "un URL non e' un percorso");
    assert!(percorso_assoluto("let p = radice.join(\"rules.toml\");").is_none());
    assert!(percorso_assoluto("let p = std::env::temp_dir().join(\"prove\");").is_none());
}
