# 3Rivals [Non finito]

**Get better or get Outclassed!**

## La cintura

Un binario Windows che guarda la finestra attiva una volta al secondo, la
confronta con quello che hai dichiarato di fare, e ti richiama quando esci di
pista: un suono dopo dieci secondi, una frase dopo quarantacinque, una frase più
dura dopo due minuti. Si ferma appena torni.

Niente AI, niente rete, niente finestra. Il fine è imparare a non distrarsi, non
sorvegliare.

> **Questa è la fase 1 di un progetto più grande.** 3Rivals nascerà come due
> rivali a voce che commentano il tuo lavoro e ti interrogano dopo un commit;
> quella parte userà l'API cloud con la tua chiave e non esiste ancora. La
> cintura è il primo pezzo, è autonoma, e funziona da sola.

### Cosa fa, in concreto

- **Guarda** la finestra in primo piano: nome del processo e titolo. Nient'altro.
- **Decide** con le tue regole (`rules.toml`) e con l'**intento** che scrivi a
  inizio sessione: le parole dell'intento sbloccano i titoli ambigui, così un
  tutorial su Rust dentro YouTube conta come lavoro e un video di gatti no.
- **Richiama** a tre livelli, e smette nello stesso istante in cui torni.
- **Tace** su ciò che non conosce: una finestra che nessuna regola descrive non
  genera mai un richiamo. Un falso richiamo costa più di uno mancato.
- **Registra** tutto in JSONL, e sa rileggersi: `report` e `verifica`.

### Provarla

Serve Rust 1.85+ e Windows.

```bash
cargo test --workspace                     # 47 test, meno di un secondo
cargo build --release

./target/release/rivals-belt.exe init      # prepara %APPDATA%\3Rivals
./target/release/rivals-belt.exe --intento "studio la concorrenza in Rust"
```

Durante la sessione si scrive nel terminale:

| Comando | Cosa fa |
|---|---|
| `pausa 10 caffè` | Sospende la cintura per 10 minuti. **Il motivo non è opzionale**: resta scritto nel registro |
| `riprendi` | Torna a guardare prima che la pausa scada |
| `stato` | Dove sei e da quanto |
| `esci` | Chiude la sessione e ti chiede il verdetto: il richiamo ti ha riportato in pista, o l'hai ignorato? |

Dopo:

```bash
./target/release/rivals-belt.exe report      # deviazioni, tempi di ritorno, fascia oraria peggiore
./target/release/rivals-belt.exe verifica    # i criteri di accettazione, sui numeri del registro
```

### Le frasi parlate

I richiami del secondo e terzo livello sono file wav, generati **una volta sola**
— la cintura non sintetizza niente a runtime, per partire in un secondo senza
tenere un modello in RAM. Il testo sta in `frasi/frasi.toml`, dentro la cartella
dati, e si cambia a mano.

Per generarli serve [Kokoro](https://github.com/hexgrad/kokoro) in ONNX
(`kokoro-v1.0.onnx` e `voices-v1.0.bin`) e `pip install kokoro-onnx`:

```bash
python tools/genera_frasi.py --modelli <cartella dei modelli> --prova-voci
```

Finché i wav non esistono, la cintura suona il richiamo breve e stampa il testo
della frase sul terminale: un livello di escalation non passa mai in silenzio.

## Cosa esce da questo PC

**Niente.** Zero byte lasciano la macchina, e non è una promessa: è verificabile.

| Cosa | Come lo controlli |
|---|---|
| Nessuna dipendenza di rete | `crates/rivals-belt/Cargo.toml`: `serde`, `serde_json`, `toml`. Nient'altro |
| Nessun codice di rete | `grep -rnE "TcpStream\|TcpListener\|reqwest\|ureq\|hyper\|UdpSocket" crates/ --include=*.rs` → nessun risultato |
| Nessuna lettura dello schermo | `src/finestra.rs` chiama `GetForegroundWindow`, `GetWindowTextW` e il nome del processo. Non esiste nessuna cattura schermo |
| Nessun percorso di sviluppo nei file | c'è un test che fallisce: `tests/nessun_percorso_assoluto.rs` |

Quello che resta **sul tuo disco**, in `%APPDATA%\3Rivals`:

| File | Contiene |
|---|---|
| `rules.toml` | Le tue regole e le soglie |
| `registro/*.jsonl` | Cambi di finestra, deviazioni, richiami, pause col motivo, i tuoi verdetti |
| `frasi/`, `suoni/` | I richiami |

**Il registro contiene i titoli delle tue finestre**, cioè i nomi dei file su cui
lavori e i titoli delle pagine che apri. Non esce di lì, ma è in chiaro: se
presti il PC, è la cartella da cancellare.

Quando arriverà la parte cloud, la regola dichiarata è questa: nessuna immagine
nel payload (bloccata nel codice, non nel prompt), ogni payload inviato salvato
in chiaro in locale, un tetto di costo, e **la cintura continua a funzionare
anche quando i rivali tacciono**.

## Com'è fatta

```
crates/rivals-belt/src/
  finestra.rs    il sensore: GetForegroundWindow, titolo, nome del processo
  regole.rs      rules.toml, l'intento, i tre verdetti: in pista / fuori / incerto
  cintura.rs     la macchina a stati: soglie, escalation, ritorno, pausa
  suono.rs       PlaySoundW, scelta della frase, generazione del richiamo breve
  registro.rs    le righe JSONL, con versione di schema
  report.rs      il report di giornata e la verifica dell'accettazione
  percorsi.rs    dove stanno i file — niente è scritto nel codice
  tempo.rs       due orologi: monotono per le durate, ora locale per le fasce
```

Tre dipendenze in tutto. Le chiamate a Windows (`user32`, `kernel32`, `winmm`)
sono dichiarate a mano: sei funzioni non valgono mezzo gigabyte di binding.

La macchina a stati della cintura è **pura** — entrano un istante e un verdetto,
escono delle azioni — e per questo mezz'ora di lavoro, due ore di distrazione e
una pausa si provano in millisecondi. È il motivo per cui il criterio "dal cambio
di finestra al suono passano al massimo soglia + 1,5 s" è un test e non un
cronometro in mano.

## Origine

3Rivals è un progetto indipendente, dello stesso autore di
[AUI](https://github.com/TheMackerel/Agentic-User-Interface-Engine-AUI-). Tutto il
codice di questo repo è scritto per 3Rivals.

I riferimenti come `WO-01` nei commenti puntano al piano di lavoro interno del
progetto: ogni work order ha i suoi criteri di accettazione, e i test li
verificano.

## Licenza

Doppia, come da convenzione Rust: [MIT](LICENSE-MIT) **oppure**
[Apache 2.0](LICENSE-APACHE), a scelta di chi usa.
