//! La cintura non ha dipendenze di rete. Mai.
//!
//! Il README lo promette e mostra un `grep` come prova. Ma un `grep` sul
//! sorgente non vede le dipendenze: il giorno che una libreria HTTP entrasse
//! di striscio, come dipendenza di una dipendenza, il `grep` resterebbe a zero
//! e la promessa sarebbe falsa. Questo test chiede a Cargo l'albero **intero**
//! delle dipendenze di `rivals-belt` e fallisce se ci trova un pezzo di rete.

use std::process::Command;

/// Le librerie con cui un programma Rust parla in rete, direttamente o
/// attraverso un client. Se una di queste compare nell'albero, qualcosa della
/// cintura *potrebbe* aprire una connessione.
const RETE: &[&str] = &[
    "reqwest", "ureq", "hyper", "h2", "http", "isahc", "curl", "attohttpc", "surf",
    "tokio", "async-std", "mio", "socket2", "rustls", "native-tls", "openssl",
    "tungstenite", "websocket",
];

/// Le sole dipendenze dirette ammesse. Aggiungerne una deve essere una scelta,
/// e questo test e' il posto dove la si fa.
const DIRETTE_AMMESSE: &[&str] = &["serde", "serde_json", "toml"];

fn albero(profondita: Option<u32>) -> Vec<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.args(["tree", "-p", "rivals-belt", "-e", "normal", "--prefix", "none", "--format", "{p}"]);
    if let Some(d) = profondita {
        cmd.args(["--depth", &d.to_string()]);
    }
    let out = cmd
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree deve poter girare");
    assert!(out.status.success(), "cargo tree fallito: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout)
        .lines()
        // "serde_json v1.0.151" -> "serde_json"; le righe "(*)" sono duplicati.
        .filter_map(|r| r.split_whitespace().next().map(str::to_string))
        .collect()
}

#[test]
fn nessuna_libreria_di_rete_nell_albero_delle_dipendenze() {
    let tutte = albero(None);
    assert!(tutte.len() > 3, "l'albero deve contenere almeno la cintura e le sue tre dipendenze: {tutte:?}");
    let colpevoli: Vec<&String> = tutte.iter().filter(|p| RETE.contains(&p.as_str())).collect();
    assert!(
        colpevoli.is_empty(),
        "la cintura ha dipendenze di rete: {colpevoli:?}. Se servono, il codice va in un altro crate."
    );
}

#[test]
fn le_dipendenze_dirette_sono_solo_quelle_dichiarate() {
    let dirette: Vec<String> = albero(Some(1)).into_iter().filter(|p| p != "rivals-belt").collect();
    for d in &dirette {
        assert!(
            DIRETTE_AMMESSE.contains(&d.as_str()),
            "dipendenza nuova non dichiarata: `{d}`. Aggiungila a DIRETTE_AMMESSE solo di proposito."
        );
    }
}
