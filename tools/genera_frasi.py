"""
genera_frasi.py — i wav dei richiami parlati della cintura (WO-01 §6).

Si lancia UNA VOLTA. La cintura non sintetizza niente a runtime: legge
`frasi.toml` e suona i file che questo script ha prodotto. E' il motivo per cui
il binario parte in un secondo e non tiene un modello in RAM.

    python tools/genera_frasi.py --modelli <cartella con kokoro-v1.0.onnx>

Senza `--modelli` cerca in `$KOKORO_MODELS`. Senza `--uscita` scrive nella
cartella dati della cintura: `%APPDATA%\\3Rivals\\frasi`, o `$RIVALS_DATA\\frasi`.

Perche' chiama Kokoro direttamente e non un server: per dodici file una
tantum, avviare un processo, occupare una porta e parlarci in JSON e' piu'
codice — e piu' cose che si rompono.

Con `--prova-voci` genera anche, in `frasi/prova_voci/`, la stessa frase detta
da tutte e due le voci italiane, per sceglierle ascoltando.
"""

from __future__ import annotations

import argparse
import os
import re
import struct
import sys
from pathlib import Path

# Le uniche due voci italiane di Kokoro v1.0.
VOCI_ITALIANE = ["if_sara", "im_nicola"]
FRASE_DI_PROVA = "Questa finestra non c'entra con quello che hai detto di fare."


def terminale_in_utf8() -> None:
    """La console di Windows parla ancora cp1252: senza questa riga, una freccia
    o un accento nel testo di una frase fanno morire lo script a meta' lavoro."""
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except (AttributeError, OSError):
        pass


def cartella_dati() -> Path:
    if os.environ.get("RIVALS_DATA"):
        return Path(os.environ["RIVALS_DATA"])
    if os.environ.get("APPDATA"):
        return Path(os.environ["APPDATA"]) / "3Rivals"
    return Path("dati-rivals")


def leggi_frasi(manifest: Path) -> list[tuple[str, str]]:
    """(file, testo) da frasi.toml.

    Lettura a espressioni regolari invece di un parser TOML: e' uno script
    d'appoggio, il file lo scriviamo noi, e una dipendenza in piu' su una
    cartella di strumenti non si giustifica. Se un giorno il manifest diventa
    complicato, questo e' il punto in cui si passa a `tomllib`.
    """
    testo = manifest.read_text(encoding="utf-8")
    blocchi = re.split(r"\[\[frase\]\]", testo)[1:]
    fuori = []
    for b in blocchi:
        file = re.search(r'file\s*=\s*"([^"]+)"', b)
        frase = re.search(r'testo\s*=\s*"([^"]+)"', b)
        if file and frase:
            fuori.append((file.group(1), frase.group(1)))
    return fuori


def scrivi_wav(path: Path, campioni, frequenza: int) -> None:
    """PCM 16 bit mono. Stesso formato che `PlaySoundW` si aspetta dalla cintura."""
    import numpy as np

    dati = (np.clip(np.asarray(campioni, dtype="float32"), -1.0, 1.0) * 32767).astype("<i2").tobytes()
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as f:
        f.write(b"RIFF")
        f.write(struct.pack("<I", 36 + len(dati)))
        f.write(b"WAVEfmt ")
        f.write(struct.pack("<IHHIIHH", 16, 1, 1, frequenza, frequenza * 2, 2, 16))
        f.write(b"data")
        f.write(struct.pack("<I", len(dati)))
        f.write(dati)


def main() -> int:
    terminale_in_utf8()
    p = argparse.ArgumentParser(description="Genera i wav dei richiami della cintura")
    p.add_argument("--modelli", default=os.environ.get("KOKORO_MODELS", ""),
                   help="cartella con kokoro-v1.0.onnx e voices-v1.0.bin")
    p.add_argument("--uscita", default="", help="cartella frasi (default: cartella dati della cintura)")
    p.add_argument("--voce", default="if_sara", choices=VOCI_ITALIANE)
    p.add_argument("--velocita", type=float, default=1.0)
    p.add_argument("--prova-voci", action="store_true", help="genera anche il confronto fra le due voci")
    a = p.parse_args()

    if not a.modelli:
        print("Serve --modelli (o $KOKORO_MODELS): la cartella con kokoro-v1.0.onnx e voices-v1.0.bin.")
        return 2
    modelli = Path(a.modelli)
    onnx, voci = modelli / "kokoro-v1.0.onnx", modelli / "voices-v1.0.bin"
    if not onnx.is_file() or not voci.is_file():
        print(f"In {modelli} non ci sono kokoro-v1.0.onnx e voices-v1.0.bin.")
        return 2

    uscita = Path(a.uscita) if a.uscita else cartella_dati() / "frasi"
    manifest = uscita / "frasi.toml"
    if not manifest.is_file():
        print(f"Manca {manifest}. Lancia prima `rivals-belt init`.")
        return 2

    from kokoro_onnx import Kokoro

    kokoro = Kokoro(str(onnx), str(voci))
    frasi = leggi_frasi(manifest)
    print(f"{len(frasi)} frasi da {manifest}, voce {a.voce} → {uscita}")

    for nome, testo in frasi:
        campioni, frequenza = kokoro.create(testo, voice=a.voce, speed=a.velocita, lang="it")
        destinazione = uscita / nome
        scrivi_wav(destinazione, campioni, int(frequenza))
        durata = len(campioni) / float(frequenza)
        print(f"  {nome:<10} {durata:4.1f} s  {testo}")

    if a.prova_voci:
        print("\nLa stessa frase con le due voci italiane:")
        for voce in VOCI_ITALIANE:
            campioni, frequenza = kokoro.create(FRASE_DI_PROVA, voice=voce, speed=1.0, lang="it")
            destinazione = uscita / "prova_voci" / f"{voce}.wav"
            scrivi_wav(destinazione, campioni, int(frequenza))
            print(f"  {destinazione}")

    print("\nFatto. La cintura le usa al prossimo richiamo, senza riavviare niente.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
